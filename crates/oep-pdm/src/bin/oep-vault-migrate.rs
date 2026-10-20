//! oep-vault-migrate -- P10-M1-T03 migration tooling.
//!
//! Imports a V5-era **folder tree** (a root directory of `CATPart` /
//! `CATProduct` / `CATDrawing` / aux files, organized into nested folders with
//! no PDM metadata) into an `oep-pdm` vault **with metadata mapping**, not a
//! dumb file copy. The mapping is exactly the one anticipated and defined by
//! the P8-M1-T02 schema (`docs/pdm/service/vault-object-model.md`, modules
//! `oep_pdm::legacy` / `oep_pdm::blobstore` / `oep_pdm::model`):
//!
//! - every **directory** becomes a `folder` [`oep_pdm::model::Context`] nested
//!   under its parent directory's context; the tree root becomes a `product`
//!   context, so the vault is browsable *as it was on disk*;
//! - every **file** becomes an [`oep_pdm::model::Item`] (kind from extension),
//!   a first [`oep_pdm::model::Revision`] (`"1"`, `working`), a
//!   [`oep_pdm::model::File`] record whose bytes are content-addressed by the
//!   file's real sha-256, and an [`oep_pdm::model::Origin`] provenance record
//!   carrying the absolute legacy path + collected-at + source hash;
//! - the run appends one [`oep_pdm::model::ImportLedger`] row whose `keys`
//!   (sha-256 of the normalized relative path) make re-runs idempotent: a
//!   re-run skips every already-imported file (`not_yet_imported` reports the
//!   delta).
//!
//! Fidelity level: this tool maps **tree shape** (metadata, observe-don't-copy).
//! It never decodes proprietary file bytes and it does NOT claim assembly
//! composition (a `.CATProduct`'s component references need the CATProduct
//! reader lane, P5/P6) -- `composed_of` edges stay out of scope by design, per
//! the schema's section 1.6 handoff note.
//!
//! Commands:
//!
//! ```text
//! oep-vault-migrate discover <root>                      list entries
//! oep-vault-migrate plan    <root> [--vault <dir>]       dry-run: print the
//!                                                        mapping, write nothing
//! oep-vault-migrate import  <root> --vault <dir> ...     write blobs + vault.json
//! oep-vault-migrate verify  <root> --vault <dir>         checksums vs source
//! oep-vault-migrate --version
//! ```
//!
//! Common flags: `--importer <id>` (default `oep-vault-migrate/0.1`),
//! `--collected-at <RFC3339>` (default: now, UTC).
//!
//! Vault layout written by `import`:
//!
//! ```text
//! <vault>/vault.json   the VaultDoc (canonical pretty JSON, oep-vault v1)
//! <vault>/blobs/<sha>  one content-addressed blob per distinct file
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oep_pdm::blobstore::{sha256_hex, BlobStore};
use oep_pdm::legacy::{self, LegacyEntry};
use oep_pdm::model::VaultDoc;

/// Importer identifier stamped into `Origin`/ledger rows (schema example).
const IMPORTER_DEFAULT: &str = "oep-vault-migrate/0.1";
/// Name of the vault-metadata document inside a vault directory.
const VAULT_FILENAME: &str = "vault.json";

/// How the invocation was parsed.
enum Cmd {
    Discover,
    Plan,
    Import,
    Verify,
}

/// Parsed command-line options.
struct Opts {
    /// The subcommand to run.
    cmd: Cmd,
    /// Root of the legacy folder tree.
    root: PathBuf,
    /// Vault directory (`--vault`); required for import/verify.
    vault: Option<PathBuf>,
    /// Importer identifier.
    importer: String,
    /// RFC 3339 collection timestamp override.
    collected_at: Option<String>,
}

const USAGE: &str = "\
oep-vault-migrate -- import V5-era folder trees into an oep-pdm vault with metadata mapping

USAGE:
  oep-vault-migrate discover <root>
  oep-vault-migrate plan <root> [--vault <dir>] [--importer <id>] [--collected-at <RFC3339>]
  oep-vault-migrate import <root> --vault <dir> [--importer <id>] [--collected-at <RFC3339>]
  oep-vault-migrate verify <root> --vault <dir>
  oep-vault-migrate --version

COMMANDS:
  discover   list the legacy files discovered under <root> (no writes)
  plan       DRY-RUN: print the item/revision/file/context mapping for the
             delta not yet imported, write NOTHING. --vault may point at an
             existing vault to report already-imported skips.
  import     REAL import: write content-addressed blobs + vault.json under
             --vault, append one import-ledger row, verify checksums against
             source bytes. Idempotent (re-runs import only the delta).
  verify     Compare every source file's bytes to its vault blob and Origin
             sha-256; re-hash every blob in the store. Exit 0 iff all match.

FLAGS:
  --importer <id>        importer identifier stamped into provenance
  --collected-at <TS>    RFC 3339 collection time (default: now, UTC)";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("oep-vault-migrate: error: {msg}");
            eprintln!("run 'oep-vault-migrate --help' for usage");
            ExitCode::from(1)
        }
    }
}

/// Run the parsed invocation, returning a process exit code.
fn run(args: Vec<String>) -> Result<u8, String> {
    if args.len() == 1 && (args[0] == "-h" || args[0] == "--help" || args[0] == "help") {
        println!("{USAGE}");
        return Ok(0);
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("oep-vault-migrate {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    let opts = parse(&args)?;
    match opts.cmd {
        Cmd::Discover => {
            let entries = legacy::discover(&opts.root).map_err(|e| e.to_string())?;
            println!("{:>4}  {:<8}  {:<10}  relative path", "n", "kind", "format");
            for (i, e) in entries.iter().enumerate() {
                println!(
                    "{:>4}  {:<8}  {:<10}  {}",
                    i + 1,
                    e.item_kind,
                    e.format,
                    e.relative
                );
            }
            println!();
            println!(
                "discovered {} file(s) under {}",
                entries.len(),
                opts.root.display()
            );
            Ok(0)
        }
        Cmd::Plan => plan_dry_run(&opts),
        Cmd::Import => run_import(&opts),
        Cmd::Verify => run_verify(&opts),
    }
}

/// Parse the raw arguments into [`Opts`], rejecting unknown forms loudly.
fn parse(args: &[String]) -> Result<Opts, String> {
    let mut cmd: Option<Cmd> = None;
    let mut root: Option<PathBuf> = None;
    let mut vault: Option<PathBuf> = None;
    let mut importer = IMPORTER_DEFAULT.to_string();
    let mut collected_at: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "discover" | "plan" | "import" | "verify" => {
                if cmd.is_some() {
                    return Err(format!("unexpected extra command '{}'", a));
                }
                cmd = Some(match a.as_str() {
                    "discover" => Cmd::Discover,
                    "plan" => Cmd::Plan,
                    "import" => Cmd::Import,
                    _ => Cmd::Verify,
                });
            }
            "--vault" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--vault needs a directory argument".to_string())?;
                vault = Some(PathBuf::from(v));
                i += 1;
            }
            "--importer" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--importer needs an identifier argument".to_string())?;
                importer = v.clone();
                i += 1;
            }
            "--collected-at" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--collected-at needs an RFC3339 argument".to_string())?;
                collected_at = Some(v.clone());
                i += 1;
            }
            other => {
                if other.starts_with("--") {
                    return Err(format!("unknown flag '{}'", other));
                }
                if root.is_some() {
                    return Err(format!("unexpected positional argument '{}'", other));
                }
                root = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    let cmd = cmd.ok_or_else(|| "missing command (discover|plan|import|verify)".to_string())?;
    let root = root.ok_or_else(|| "missing <root> directory argument".to_string())?;
    Ok(Opts {
        cmd,
        root,
        vault,
        importer,
        collected_at,
    })
}

/// RFC 3339 UTC timestamp of now, std-only (no chrono dependency).
fn rfc3339_utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;

    // Civil-from-days (Howard Hinnant's algorithm): seconds -> (y, m, d, h, m, s).
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Load the vault document at `<dir>/vault.json`, if it exists.
fn load_vault(dir: &Path) -> Result<Option<VaultDoc>, String> {
    let path = dir.join(VAULT_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let doc = VaultDoc::from_json_pretty(&text)
        .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    Ok(Some(doc))
}

/// Atomically write `<dir>/vault.json` (temp file + rename, like the blob store).
fn write_vault_atomic(dir: &Path, doc: &VaultDoc) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let text = doc
        .to_json_pretty()
        .map_err(|e| format!("cannot serialize vault: {e}"))?;
    let final_path = dir.join(VAULT_FILENAME);
    let tmp = dir.join(format!(".{}.tmp-{}", VAULT_FILENAME, std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &final_path)
        .map_err(|e| format!("cannot move {} into place: {e}", final_path.display()))?;
    Ok(())
}

/// The collection timestamp: override or now.
fn collected_at(opts: &Opts) -> String {
    opts.collected_at.clone().unwrap_or_else(rfc3339_utc_now)
}

/// Shared discovery + delta planning for `plan` and `import`.
fn discover_and_plan(
    opts: &Opts,
) -> Result<(Vec<LegacyEntry>, VaultDoc, legacy::ImportPlan, usize), String> {
    let entries = legacy::discover(&opts.root).map_err(|e| e.to_string())?;
    let base = match &opts.vault {
        Some(dir) => load_vault(dir)?.unwrap_or_else(VaultDoc::new),
        None => VaultDoc::new(),
    };
    let (plan, skipped) = legacy::plan_delta(
        &base,
        &opts.root,
        &entries,
        &collected_at(opts),
        &opts.importer,
    )
    .map_err(|e| e.to_string())?;
    Ok((entries, base, plan, skipped))
}

/// Print one mapping row for the dry-run / summary.
fn print_row(e: &LegacyEntry, item_id: &str, rev_id: &str, blob: &str) {
    println!(
        "  {:<45}  ->  item {:<20} {}  rev {:<2}  {}  blob {}",
        e.relative,
        item_id,
        e.item_kind,
        rev_id,
        e.format,
        short(blob)
    );
}

/// First 12 hex chars of a content address, for compact display.
fn short(sha: &str) -> String {
    sha.chars().take(16).collect()
}

/// DRY-RUN: print the mapping for the delta; write nothing.
fn plan_dry_run(opts: &Opts) -> Result<u8, String> {
    let (entries, _base, plan, skipped) = discover_and_plan(opts)?;
    let doc = &plan.doc;

    println!("dry-run plan for legacy tree {}", opts.root.display());
    println!(
        "  discovered {} file(s), {} already imported (skipped)",
        entries.len(),
        skipped
    );
    println!(
        "  would create {} context(s), {} item(s), {} revision(s), {} file(s)",
        doc.contexts.len(),
        doc.items.len(),
        doc.revisions.len(),
        doc.files.len()
    );
    println!("mapping (relative path -> vault object):");

    for (i, e) in entries.iter().enumerate() {
        if i < doc.items.len() {
            let item = &doc.items[i];
            let rev = doc
                .revisions
                .iter()
                .find(|r| r.item_id == item.id)
                .unwrap_or_else(|| panic!("planned revision missing for item"));
            let file = doc
                .files
                .iter()
                .find(|f| rev.file_ids.first().map(|id| id == &f.id).unwrap_or(false))
                .unwrap_or_else(|| panic!("planned file missing for revision"));
            print_row(e, &item.id, &rev.id, &file.blob_sha256);
        } else {
            println!("  {:<45}  ->  [already imported; skipped]", e.relative);
        }
    }

    let errors = doc.validate();
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("  validation: {}: {}", err.code, err.message);
        }
        // A planned migration producing an invalid vault is a hard failure; the
        // import would refuse to write it, and the dry-run says so.
        return Err(format!(
            "planned vault is invalid ({} violation(s)); nothing written",
            errors.len()
        ));
    }

    println!();
    println!("dry-run: no files written. Re-run with 'import' to write the vault.");
    Ok(0)
}

/// REAL import: write blobs + vault.json for the delta; verify against source.
fn run_import(opts: &Opts) -> Result<u8, String> {
    let vault = opts
        .vault
        .clone()
        .ok_or_else(|| "import requires --vault <dir>".to_string())?;
    let (entries, base, plan, skipped) = discover_and_plan(opts)?;

    if plan.doc.items.is_empty() {
        println!(
            "nothing to import: all {} file(s) already present in the vault ledger ({} skipped)",
            entries.len(),
            skipped
        );
        return Ok(0);
    }

    // 1) Store blobs first. plan.blob_addresses is aligned with plan.doc.files
    //    order, which is the delta entries order (plan_import pushes per entry).
    let store = BlobStore::new(&vault);
    let delta_entries: Vec<&LegacyEntry> = entries
        .iter()
        .filter(|e| {
            plan.doc
                .imports
                .first()
                .map(|row| row.keys.contains(&legacy::ledger_key_for(&e.relative)))
                .unwrap_or(false)
        })
        .collect();

    // Recompute the addressed blobs from the real source bytes (never trust a
    // stored address blindly) and confirm the plan's addresses match.
    let mut stored = Vec::new();
    for (e, addr) in delta_entries.iter().zip(&plan.blob_addresses) {
        let bytes = std::fs::read(&e.path)
            .map_err(|err| format!("cannot read {}: {err}", e.path.display()))?;
        let real = sha256_hex(&bytes);
        if &real != addr {
            return Err(format!(
                "plan/bytes mismatch for {}: planned {}, source hashes to {}",
                e.relative, addr, real
            ));
        }
        let wrote = store
            .put(&bytes)
            .map_err(|err| format!("blob store write failed for {}: {err}", e.relative))?;
        stored.push((e.relative.clone(), wrote));
    }

    // 2) Merge the delta into the existing vault (idempotent by id) and append
    //    the run's ledger row via plan_delta's plan (plan.doc.imports).
    let mut final_doc = base.clone();
    let counts = legacy::merge_vault(&mut final_doc, &plan.doc);

    // 3) Never write an invalid vault: validate before touching disk.
    let errors = final_doc.validate();
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("  validation: {}: {}", err.code, err.message);
        }
        return Err(format!(
            "refusing to write an invalid vault ({} violation(s))",
            errors.len()
        ));
    }

    // 4) Write the vault-metadata document (atomic), then prove every blob the
    //    final vault references is present and hash-correct.
    write_vault_atomic(&vault, &final_doc)?;
    store
        .verify_doc_files(&final_doc)
        .map_err(|err| format!("post-import blob verification failed: {err}"))?;

    // 5) Checksums verified AGAINST SOURCE: each stored blob must byte-match the
    //    legacy source file it came from.
    for (rel, addr) in &stored {
        let entry = delta_entries
            .iter()
            .find(|e| &e.relative == rel)
            .expect("delta entry present");
        let src = std::fs::read(&entry.path)
            .map_err(|err| format!("cannot re-read {}: {err}", entry.path.display()))?;
        let got = store
            .get(addr)
            .map_err(|err| format!("cannot read stored blob {addr}: {err}"))?;
        if got != src {
            return Err(format!(
                "checksum vs source FAILED for {}: stored blob does not match source bytes",
                rel
            ));
        }
    }

    println!(
        "imported {} file(s) from {} -> vault at {}",
        delta_entries.len(),
        opts.root.display(),
        vault.display()
    );
    println!(
        "  appended: {} context(s), {} item(s), {} revision(s), {} file(s); {} blob(s) stored; {} already-imported skipped",
        counts.contexts, counts.items, counts.revisions, counts.files, stored.len(), skipped
    );
    println!(
        "  checksums verified against source: {}/{}/{} ok",
        stored.len(),
        stored.len(),
        stored.len()
    );
    println!(
        "  vault document: {} (oep-vault v1, validated clean)",
        vault.join(VAULT_FILENAME).display()
    );
    Ok(0)
}

/// VERIFY: compare every source file to its vault blob + Origin sha-256, and
/// re-hash every blob in the store. Exit 0 iff everything matches.
fn run_verify(opts: &Opts) -> Result<u8, String> {
    let vault = opts
        .vault
        .clone()
        .ok_or_else(|| "verify requires --vault <dir>".to_string())?;
    if !vault.is_dir() {
        return Err(format!(
            "vault directory does not exist: {}",
            vault.display()
        ));
    }
    let doc = load_vault(&vault)?
        .ok_or_else(|| format!("no {} in {}", VAULT_FILENAME, vault.display()))?;
    let structural = doc.validate();
    if !structural.is_empty() {
        for err in &structural {
            eprintln!("  validation: {}: {}", err.code, err.message);
        }
        return Err(format!(
            "vault document is invalid ({} violation(s))",
            structural.len()
        ));
    }

    let store = BlobStore::new(&vault);
    // Re-hash every blob in the store (whole-store integrity).
    let report = store.verify();
    println!(
        "blob store scan: {} ok, {} corrupt/missing",
        report.ok, report.corrupt
    );
    if report.corrupt > 0 {
        eprintln!(
            "  first problem: {}",
            report
                .first_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
        return Err("blob store has corrupt or missing blobs".to_string());
    }

    let entries = legacy::discover(&opts.root).map_err(|e| e.to_string())?;
    let mut pass = 0usize;
    let mut fail = 0usize;
    for e in &entries {
        let src = match std::fs::read(&e.path) {
            Ok(b) => b,
            Err(err) => {
                println!("  FAIL  {:<45}  unreadable source: {err}", e.relative);
                fail += 1;
                continue;
            }
        };
        let src_sha = sha256_hex(&src);

        // Locate the vault record: item whose Origin legacy_path ends with this
        // entry's relative path (POSIX-normalized), so verify works from any
        // machine even when absolute paths differ.
        let norm = |p: &str| p.replace('\\', "/");
        let item = doc.items.iter().find(|i| {
            i.origin
                .as_ref()
                .map(|o| norm(&o.legacy_path).ends_with(&norm(&e.relative)))
                .unwrap_or(false)
        });

        let Some(item) = item else {
            println!(
                "  MISSING  {:<45}  no vault item with Origin for this path; run 'import' first",
                e.relative
            );
            fail += 1;
            continue;
        };

        let mut ok = true;
        let mut why = String::new();

        // Origin sha-256 cross-check.
        if ok {
            if let Some(sha) = item.origin.as_ref().and_then(|o| o.sha256.clone()) {
                if sha != src_sha {
                    ok = false;
                    why = format!(
                        "Origin sha-256 mismatch ({} vs source {})",
                        short(&sha),
                        short(&src_sha)
                    );
                }
            }
        }

        // Blob byte-compare against the revision's file record.
        if ok {
            let rev = doc
                .revisions
                .iter()
                .find(|r| r.item_id == item.id)
                .ok_or_else(|| format!("no revision for item {}", item.id))?;
            let file_id = rev.file_ids.first();
            match file_id.and_then(|fid| doc.files.iter().find(|f| f.id == *fid)) {
                Some(file) => match store.get(&file.blob_sha256) {
                    Ok(bytes) => {
                        if bytes != src {
                            ok = false;
                            why = format!(
                                "vault blob {} does not match source bytes",
                                short(&file.blob_sha256)
                            );
                        }
                    }
                    Err(err) => {
                        ok = false;
                        why = format!("stored blob unavailable: {err}");
                    }
                },
                None => {
                    ok = false;
                    why = "revision has no file record".to_string();
                }
            }
        }

        if ok {
            println!(
                "  PASS  {:<45}  blob {} == source",
                e.relative,
                short(&src_sha)
            );
            pass += 1;
        } else {
            println!("  FAIL  {:<45}  {why}", e.relative);
            fail += 1;
        }
    }

    if fail > 0 {
        return Err(format!(
            "verify failed: {} ok, {} failed/missing",
            pass, fail
        ));
    }
    println!();
    println!(
        "verify PASS: {} source file(s) match their vault blobs (+{} store scan)",
        pass, report.ok
    );
    println!("verify FAIL count: 0");
    Ok(0)
}

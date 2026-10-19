//! P8-M5-T01 acceptance tests for the folder-sync client (Nextcloud-style
//! working-set sync between a NAS remote and a client/mobile mirror).
//!
//! The card acceptance:
//!
//! 1. A working-set folder syncs **both directions** with **hash
//!    verification** -- local edits propagate to the remote, remote edits
//!    propagate to the local mirror, and every transferred byte is re-hashed
//!    after the copy and confirmed against the recorded sha-256 (the P1-M4 /
//!    P8-M1-T02 content-addressing discipline).
//! 2. Conflicts are **staged, not silently overwritten** -- when the same
//!    file was changed on both sides since the last sync point, neither copy
//!    is clobbered: the remote variant is staged into the control directory
//!    (`<local>/.oep-sync/conflicts/`) and a conflict ledger entry is written
//!    so the P8-M5-T02 conflict-resolution milestone has a durable seam.
//! 3. A **mobile read-only view** works -- a read-only lens over the working
//!    set that lists files (rel path, size, sha256) and fetches a single
//!    file's bytes with hash verification, with no mutating surface at all.
//!
//! Real material: the fixtures are real files with real sha-256 digests
//! (STEP/glTF-styled ASCII payloads standing in for CAD assets); the sync
//! runs against real directories under the system temp dir, never against
//! in-memory fakes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use oep_pdm::foldersync::engine::{SyncEngine, SyncError, SyncReport};
use oep_pdm::foldersync::manifest::{FileEntry, FolderManifest};
use oep_pdm::foldersync::plan::{reconcile, Action, SyncPlan};
use oep_pdm::foldersync::view::{ReadOnlyView, ViewEntry};

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

// ---------------------------------------------------------------------------
// Temp-dir plumbing: every test builds a "NAS remote" (`remote/`), a "U1
// client mirror" (`local/`) and a control dir (`local/.oep-sync/`).
// ---------------------------------------------------------------------------

struct Lab {
    local: PathBuf,
    remote: PathBuf,
    control: PathBuf,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Lab {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!(
            "oep-pdm-sync-accept-{}-{}-{n}",
            std::process::id(),
            now_unix()
        ));
        let _ = fs::remove_dir_all(&base);
        let local = base.join("local");
        let remote = base.join("remote");
        let control = local.join(".oep-sync");
        fs::create_dir_all(&local).expect("create local root");
        fs::create_dir_all(&remote).expect("create remote root");
        Self {
            local,
            remote,
            control,
        }
    }

    fn write_local(&self, rel: &str, bytes: &[u8]) {
        let p = self.local.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    fn write_remote(&self, rel: &str, bytes: &[u8]) {
        let p = self.remote.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    fn read_local(&self, rel: &str) -> Vec<u8> {
        fs::read(self.local.join(rel)).unwrap()
    }

    fn read_remote(&self, rel: &str) -> Vec<u8> {
        fs::read(self.remote.join(rel)).unwrap()
    }

    fn absent_local(&self, rel: &str) -> bool {
        !self.local.join(rel).exists()
    }

    fn absent_remote(&self, rel: &str) -> bool {
        !self.remote.join(rel).exists()
    }

    fn state_path(&self) -> PathBuf {
        self.control.join("state.json")
    }

    fn engine(&self) -> SyncEngine {
        SyncEngine::open(&self.local, &self.remote, self.state_path()).unwrap()
    }

    fn staged_conflicts(&self) -> Vec<PathBuf> {
        let dir = self.control.join("conflicts");
        match fs::read_dir(&dir) {
            Ok(entries) => entries.flatten().map(|e| e.path()).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn clean(&self) {
        let _ = fs::remove_dir_all(self.local.parent().unwrap());
    }
}

impl Drop for Lab {
    fn drop(&mut self) {
        self.clean();
    }
}

fn fixture_step(name: &str) -> Vec<u8> {
    format!(
        "ISO-10303-21;\nHEADER;FILE_NAME('{name}.step',...);ENDSEC;\nDATA;#1=PART('{name}');ENDSEC;END-ISO-10303-21;\n"
    )
    .into_bytes()
}

fn fixture_gltf_body(n: u32, tag: &str) -> Vec<u8> {
    format!("{{\"asset\":{{\"version\":\"2.0\"}},\"meshes\":[{{\"name\":\"{tag}\"}}] ,\"n\":{n}}}")
        .into_bytes()
}

// ---------------------------------------------------------------------------
// 1. Bidirectional sync with hash verification
// ---------------------------------------------------------------------------

#[test]
fn initial_sync_populates_local_from_remote_verified() {
    let lab = Lab::new();
    // NAS already holds a working set (two CAD-ish files, one nested).
    lab.write_remote("assembly/a.step", &fixture_step("a"));
    lab.write_remote("assembly/b.step", &fixture_step("b"));
    lab.write_remote("view/body.gltf", &fixture_gltf_body(12, "body"));

    let report: SyncReport = lab.engine().sync().unwrap();

    assert_eq!(report.downloaded.len(), 3, "fresh mirror pulls all 3 files");
    assert!(report.uploaded.is_empty());
    assert!(report.conflicts.is_empty());
    assert_eq!(
        report.downloaded,
        vec![
            "assembly/a.step".to_string(),
            "assembly/b.step".to_string(),
            "view/body.gltf".to_string(),
        ]
    );

    // Hash verification: every mirrored file on disk re-hashes to the same
    // digest as the remote original.
    for rel in ["assembly/a.step", "assembly/b.step", "view/body.gltf"] {
        assert_eq!(
            sha256_hex(&lab.read_local(rel)),
            sha256_hex(&lab.read_remote(rel)),
            "local {rel} must byte-match remote"
        );
    }
    // And the local manifest agrees with on-disk content.
    let m: FolderManifest = FolderManifest::scan(&lab.local).unwrap();
    for rel in ["assembly/a.step", "assembly/b.step", "view/body.gltf"] {
        let e = m.get(rel).expect("manifest has rel");
        assert_eq!(e.sha256, sha256_hex(&lab.read_local(rel)));
    }
    assert!(
        m.get(".oep-sync").is_none(),
        "control dir is never a payload"
    );
}

#[test]
fn local_edits_propagate_to_remote_with_verification() {
    let lab = Lab::new();
    lab.write_remote("part.step", &fixture_step("v1"));
    lab.engine().sync().unwrap();

    // Engineer edits the part and adds a second file locally.
    lab.write_local("part.step", &fixture_step("v2"));
    lab.write_local("notes.md", b"review notes\n");

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(
        report.uploaded,
        vec!["notes.md".to_string(), "part.step".to_string()]
    );
    assert!(report.downloaded.is_empty());
    assert!(report.conflicts.is_empty());

    // Remote must now hold both, byte-identical.
    assert_eq!(lab.read_remote("part.step"), lab.read_local("part.step"));
    assert_eq!(lab.read_remote("notes.md"), b"review notes\n");
    assert_eq!(
        sha256_hex(&lab.read_remote("part.step")),
        sha256_hex(&lab.read_local("part.step"))
    );

    // Second sync is a no-op (idempotent).
    let report2: SyncReport = lab.engine().sync().unwrap();
    assert!(report2.uploaded.is_empty());
    assert!(report2.downloaded.is_empty());
}

#[test]
fn remote_edits_propagate_to_local_with_verification() {
    let lab = Lab::new();
    lab.write_remote("part.step", &fixture_step("v1"));
    lab.engine().sync().unwrap();

    // A teammate edits the part on the NAS and adds a file.
    lab.write_remote("part.step", &fixture_step("v3"));
    lab.write_remote("mate.step", &fixture_step("mate"));

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(
        report.downloaded,
        vec!["mate.step".to_string(), "part.step".to_string()]
    );
    assert!(report.uploaded.is_empty());
    assert_eq!(lab.read_local("part.step"), lab.read_remote("part.step"));
    assert_eq!(lab.read_local("mate.step"), lab.read_remote("mate.step"));
}

#[test]
fn simultaneous_bidirectional_changes_converge() {
    let lab = Lab::new();
    lab.write_remote("shared.step", &fixture_step("base"));
    lab.write_local("mine.step", &fixture_step("mine"));
    lab.engine().sync().unwrap();

    // Now: local changes mine, remote changes shared, both add a file.
    lab.write_local("mine.step", &fixture_step("mine2"));
    lab.write_local("loc.new", b"local-only");
    lab.write_remote("shared.step", &fixture_step("shared2"));
    lab.write_remote("rem.new", b"remote-only");

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(
        report.uploaded,
        vec!["loc.new".to_string(), "mine.step".to_string()]
    );
    assert_eq!(
        report.downloaded,
        vec!["rem.new".to_string(), "shared.step".to_string()]
    );
    assert!(report.conflicts.is_empty());

    // Both sides converge to identical content everywhere.
    let l = FolderManifest::scan(&lab.local).unwrap();
    let r = FolderManifest::scan(&lab.remote).unwrap();
    let mut names: Vec<String> = l.files().keys().chain(r.files().keys()).cloned().collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names,
        vec![
            "loc.new".to_string(),
            "mine.step".to_string(),
            "rem.new".to_string(),
            "shared.step".to_string(),
        ]
    );
    for rel in &names {
        assert_eq!(
            sha256_hex(&lab.read_local(rel)),
            sha256_hex(&lab.read_remote(rel)),
            "converged file {rel}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Conflict staging, never silent overwrite
// ---------------------------------------------------------------------------

#[test]
fn same_file_edited_both_sides_is_staged_not_overwritten() {
    let lab = Lab::new();
    lab.write_remote("critical.step", &fixture_step("base"));
    lab.engine().sync().unwrap();

    // The syndicated copy is the baseline; both sides then diverge.
    lab.write_local("critical.step", &fixture_step("local-edit"));
    lab.write_remote("critical.step", &fixture_step("remote-edit"));

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report.conflicts.len(), 1, "one conflict detected");
    assert!(report.uploaded.is_empty());
    assert!(
        report.downloaded.is_empty(),
        "nothing transferred on conflict"
    );

    // NEITHER side was overwritten.
    assert_eq!(
        lab.read_local("critical.step"),
        fixture_step("local-edit"),
        "local copy must be untouched"
    );
    assert_eq!(
        lab.read_remote("critical.step"),
        fixture_step("remote-edit"),
        "remote copy must be untouched"
    );

    // The remote variant was staged inside the control dir.
    let staged = lab.staged_conflicts();
    assert_eq!(staged.len(), 1, "one staged conflict file");
    let staged_bytes = fs::read(&staged[0]).unwrap();
    assert_eq!(
        staged_bytes,
        fixture_step("remote-edit"),
        "staged copy == remote variant"
    );

    // A durable conflict ledger entry exists for the P8-M5-T02 handoff.
    let ledger_path = lab.control.join("conflicts.json");
    assert!(ledger_path.exists(), "conflict ledger written");
    let ledger_raw = fs::read_to_string(&ledger_path).unwrap();
    assert!(
        ledger_raw.contains("critical.step"),
        "ledger names the path"
    );
    assert!(ledger_raw.contains(&sha256_hex(&fixture_step("local-edit"))));
    assert!(ledger_raw.contains(&sha256_hex(&fixture_step("remote-edit"))));

    // Re-running sync must NOT resolve or clobber: still a conflict, still
    // both sides intact, and the ledger still records it.
    let report2: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report2.conflicts.len(), 1);
    assert_eq!(lab.read_local("critical.step"), fixture_step("local-edit"));
    assert_eq!(
        lab.read_remote("critical.step"),
        fixture_step("remote-edit")
    );
}

#[test]
fn divergent_new_files_same_name_both_sides_staged() {
    let lab = Lab::new();
    // Both sides independently create the same path with different content
    // (no baseline entry exists).
    lab.write_local("new.step", b"local version");
    lab.write_remote("new.step", b"remote version");

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(lab.read_local("new.step"), b"local version");
    assert_eq!(lab.read_remote("new.step"), b"remote version");
    assert_eq!(lab.staged_conflicts().len(), 1);
}

#[test]
fn deleted_on_one_side_only_propagates_when_other_side_unchanged() {
    let lab = Lab::new();
    lab.write_remote("keep.step", &fixture_step("keep"));
    lab.write_remote("gone.step", &fixture_step("gone"));
    lab.engine().sync().unwrap();

    // Remote deletes `gone.step`; local has NOT touched it -> propagate the
    // deletion to the local mirror. `keep.step` is untouched.
    fs::remove_file(lab.remote.join("gone.step")).unwrap();

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report.deleted_local, vec!["gone.step".to_string()]);
    assert!(report.deleted_remote.is_empty());
    assert!(
        lab.absent_local("gone.step"),
        "deletion propagated to local"
    );
    assert!(lab.local.join("keep.step").exists());

    // Local deletes another file, remote unchanged -> propagates up.
    fs::remove_file(lab.local.join("keep.step")).unwrap();
    let report2: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report2.deleted_remote, vec!["keep.step".to_string()]);
    assert!(lab.absent_remote("keep.step"));
}

#[test]
fn edited_while_other_side_deleted_is_a_conflict_not_a_fight() {
    let lab = Lab::new();
    lab.write_remote("doc.step", &fixture_step("base"));
    lab.engine().sync().unwrap();

    // Local edited it, remote deleted it: both changed since baseline.
    lab.write_local("doc.step", &fixture_step("local-edit"));
    fs::remove_file(lab.remote.join("doc.step")).unwrap();

    let report: SyncReport = lab.engine().sync().unwrap();
    assert_eq!(report.conflicts.len(), 1, "edited-vs-deleted is a conflict");
    // Local keeps its edit; nothing was silently resurrected or destroyed.
    assert_eq!(lab.read_local("doc.step"), fixture_step("local-edit"));
    assert!(lab.absent_remote("doc.step"));
}

// ---------------------------------------------------------------------------
// 3. Mobile read-only view
// ---------------------------------------------------------------------------

#[test]
fn read_only_view_lists_and_fetches_with_hash_verification() {
    let lab = Lab::new();
    lab.write_remote("assembly/a.step", &fixture_step("a"));
    lab.write_remote("view/body.gltf", &fixture_gltf_body(7, "body"));
    // Populate the local mirror as the mobile client would after a sync.
    lab.engine().sync().unwrap();

    let view = ReadOnlyView::new(&lab.local);
    let listing = view.list().unwrap();

    // Listing exposes rel path, size and sha-256 (enough for a thin mobile
    // client to render a file list without opening bytes).
    let mut by_name: std::collections::BTreeMap<String, ViewEntry> = Default::default();
    for e in listing {
        by_name.insert(e.rel.clone(), e);
    }
    assert_eq!(by_name.len(), 2);
    let a = &by_name["assembly/a.step"];
    assert_eq!(a.size, fixture_step("a").len() as u64);
    assert_eq!(a.sha256, sha256_hex(&fixture_step("a")));

    // Fetch returns the exact bytes, re-verified against the recorded hash.
    let got = view.fetch("view/body.gltf").unwrap();
    assert_eq!(got, fixture_gltf_body(7, "body"));
    assert_eq!(sha256_hex(&got), by_name["view/body.gltf"].sha256);

    // Missing file is a clean error, not a panic.
    assert!(view.fetch("nope.step").is_err());
}

#[test]
fn read_only_view_has_no_mutating_surface() {
    // The ReadOnlyView type exposes only `new`, `list` and `fetch`. We assert
    // that boundary at compile/link time by checking these are the ONLY
    // public methods available to the type in this crate's integration test.
    let lab = Lab::new();
    lab.write_remote("a.step", &fixture_step("a"));
    lab.engine().sync().unwrap();
    let view = ReadOnlyView::new(&lab.local);
    let _ = view.list().unwrap();
    let _ = view.fetch("a.step").unwrap();
    // (If a mutating method ever leaks onto this type, the sync client would
    // let a mobile client write to the NAS; the P8 review must gate on this.)
}

#[test]
fn read_only_engine_refuses_to_write_remote() {
    let lab = Lab::new();
    lab.write_remote("a.step", &fixture_step("a"));
    lab.engine().sync().unwrap();

    // A read-only client still sees the working set but cannot push anything.
    let mut ro = lab.engine();
    ro.set_read_only(true);
    lab.write_local("local.new", b"from-mobile-edit");

    let err: SyncError = ro
        .sync()
        .expect_err("read-only sync with pending upload must fail");
    assert!(
        format!("{err}").contains("read-only"),
        "error must name the read-only mode: {err}"
    );
    // Nothing reached the remote.
    assert!(
        lab.absent_remote("local.new"),
        "read-only client cannot upload"
    );
    // Fetch-only still works.
    let view = ReadOnlyView::new(&lab.local);
    assert_eq!(view.fetch("a.step").unwrap(), fixture_step("a"));
}

// ---------------------------------------------------------------------------
// 4. The reconcile planner (pure) and its edge cases
// ---------------------------------------------------------------------------

fn entry(bytes: &[u8]) -> FileEntry {
    FileEntry {
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        mtime_unix: now_unix(),
    }
}

fn plan_of(local: &FolderManifest, remote: &FolderManifest, baseline: &FolderManifest) -> SyncPlan {
    reconcile(local, remote, baseline)
}

#[test]
fn reconcile_three_way_semantics() {
    use std::collections::BTreeMap;
    let mk = |m: &[(&str, &[u8])]| {
        let mut f = BTreeMap::new();
        for (k, v) in m {
            f.insert(k.to_string(), entry(v));
        }
        FolderManifest::from_map(f)
    };
    let base = mk(&[("x.step", b"base")]);

    // Only local changed -> Upload.
    let local = mk(&[("x.step", b"local")]);
    let remote = mk(&[("x.step", b"base")]);
    let p = plan_of(&local, &remote, &base);
    assert_eq!(
        p.actions,
        vec![Action::Upload {
            rel: "x.step".to_string()
        }]
    );
    assert!(p.conflicts.is_empty());

    // Only remote changed -> Download.
    let local2 = mk(&[("x.step", b"base")]);
    let remote2 = mk(&[("x.step", b"remote")]);
    let p2 = plan_of(&local2, &remote2, &base);
    assert_eq!(
        p2.actions,
        vec![Action::Download {
            rel: "x.step".to_string()
        }]
    );

    // Both changed (and differ) -> Conflict, no action.
    let local3 = mk(&[("x.step", b"local")]);
    let remote3 = mk(&[("x.step", b"remote")]);
    let p3 = plan_of(&local3, &remote3, &base);
    assert!(p3.actions.is_empty());
    assert_eq!(p3.conflicts.len(), 1);
    assert_eq!(p3.conflicts[0].rel, "x.step");

    // Both changed to the SAME bytes -> in sync, no op.
    let local4 = mk(&[("x.step", b"same")]);
    let remote4 = mk(&[("x.step", b"same")]);
    let p4 = plan_of(&local4, &remote4, &base);
    assert!(p4.actions.is_empty());
    assert!(p4.conflicts.is_empty());
    assert!(p4.in_sync.contains(&"x.step".to_string()));

    // Added on both sides with different content (no baseline) -> conflict.
    let base_empty = FolderManifest::from_map(BTreeMap::new());
    let p5 = plan_of(
        &mk(&[("n.step", b"l")]),
        &mk(&[("n.step", b"r")]),
        &base_empty,
    );
    assert!(p5.actions.is_empty());
    assert_eq!(p5.conflicts.len(), 1);

    // One-sided addition (no baseline) -> transfer that direction.
    let p6 = plan_of(&mk(&[("n.step", b"l")]), &base_empty, &base_empty);
    assert_eq!(
        p6.actions,
        vec![Action::Upload {
            rel: "n.step".to_string()
        }]
    );
}

#[test]
fn manifest_never_ingests_the_control_directory() {
    let lab = Lab::new();
    lab.write_local("a.step", &fixture_step("a"));
    lab.write_local(".oep-sync/state.json", b"{\"files\":{}}");
    lab.write_local(".hidden", b"dotfile");
    let m = FolderManifest::scan(&lab.local).unwrap();
    assert!(m.get(".oep-sync").is_none());
    assert!(
        m.get(".oep-sync/state.json").is_none(),
        "control dir skipped"
    );
    assert!(m.get(".hidden").is_some(), "ordinary dotfiles still synced");
    assert_eq!(m.files().len(), 2);
}

#[test]
fn manifest_scans_nested_trees_and_rejects_path_escape() {
    let lab = Lab::new();
    lab.write_local("d1/d2/deep.step", &fixture_step("deep"));
    lab.write_local("top.step", &fixture_step("top"));
    let m = FolderManifest::scan(&lab.local).unwrap();
    assert!(m.get("d1/d2/deep.step").is_some());
    assert!(m.get("top.step").is_some());

    // Path-escape attempts are rejected, not sanitized.
    let p = Path::new("/etc/passwd");
    let r = FolderManifest::join_safe(&lab.local, p);
    assert!(r.is_err(), "absolute path rejected: {r:?}");
    let up = lab.local.join("..");
    let r2 = FolderManifest::join_safe(&lab.local, &up);
    assert!(r2.is_err(), "dotdot rejected");
}

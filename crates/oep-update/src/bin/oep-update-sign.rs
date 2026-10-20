//! `oep-update-sign` -- admin / release-engine bundle signer for the
//! P10-M5-T01 update checker (decision #11, P11-T04 tie-in). Produces the
//! signed offline bundle (`BundleFile`) the protected checker consumes,
//! signed before any payload is exposed.
//!
//! Modes
//! -----
//!   single payload (legacy P10-M5-T01 CLI):
//!     oep-update-sign --out OUT.json --version X.Y.Z --payload FILE \
//!         --secret HEX64 [--product oep] [--notes "..."] [--released DATE]
//!
//!   multi-artifact signed release manifest (P11-T04 / P12 release engine):
//!     oep-update-sign --out update.bundle.json --version X.Y.Z --secret HEX64 \
//!         --inline-payload RELEASE-MANIFEST.json \
//!         --item "name:sha256:size[:url]" --item ...
//!     The inline payload (the release manifest) is pinned as payloads[0] so
//!     the checker's inline-payload integrity gate still holds; each --item
//!     names one shipped artifact with its content hash, size and optional
//!     download URL. A bare-name --item resolves sha256+size from the file in
//!     the current directory.
//!
//!   key generation (build-host secret material, never committed):
//!     oep-update-sign --gen-key DIR     # writes secret.hex + public.hex
//!
//! The secret is a 64-hex-char Ed25519 secret key. The derived public key is
//! printed on every sign so you can commit it (the checker config carries it).

use std::path::PathBuf;

use oep_update::signing::{sign_bundle, signing_public_key};
use oep_update::{sha256_hex, Manifest, PayloadMeta};

#[derive(Debug)]
struct Item {
    name: String,
    sha256: String,
    size: u64,
    url: Option<String>,
}

fn usage() -> String {
    "usage:
  oep-update-sign --out OUT.json --version X.Y.Z --secret HEX64 \\
      [--payload FILE | (--inline-payload MANIFEST [--item 'name:sha256:size[:url]'] ...)] \\
      [--product oep] [--notes ...] [--released DATE]
  oep-update-sign --gen-key DIR"
        .to_string()
}

fn parse_hex64(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64 {
        return Err("expected 64 hex chars".to_string());
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char)
            .to_digit(16)
            .ok_or_else(|| "non-hex char".to_string())? as u8;
        let lo = (chunk[1] as char)
            .to_digit(16)
            .ok_or_else(|| "non-hex char".to_string())? as u8;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// Parse `--item` (a full "name:sha256:size[:url]" spec; a prefix without the
/// full form resolves sha256+size from the named file in `cwd`).
fn parse_item(spec: &str, cwd: &std::path::Path) -> Result<Item, String> {
    let parts: Vec<&str> = spec.splitn(4, ':').collect();
    match parts.len() {
        1 => {
            // Bare name: resolve from disk.
            let path = cwd.join(spec);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("cannot read --item {}: {e}", path.display()))?;
            Ok(Item {
                name: spec.to_string(),
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
                url: None,
            })
        }
        3 | 4 => {
            let name = parts[0].to_string();
            let sha256 = parts[1].to_string();
            let size = parts[2]
                .parse::<u64>()
                .map_err(|e| format!("bad size in --item {spec}: {e}"))?;
            let url = if parts.len() == 4 {
                Some(parts[3].to_string())
            } else {
                None
            };
            Ok(Item {
                name,
                sha256,
                size,
                url,
            })
        }
        _ => Err(format!(
            "--item must be 'name' or 'name:sha256:size[:url]', got: {spec}"
        )),
    }
}

fn main() {
    let mut out = PathBuf::from("update.bundle.json");
    let mut version = String::new();
    let mut payload_path: Option<PathBuf> = None;
    let mut inline_payload: Option<PathBuf> = None;
    let mut items: Vec<Item> = Vec::new();
    let mut secret_hex = String::new();
    let mut product = "oep".to_string();
    let mut notes: Option<String> = None;
    let mut released = "2026-10-17".to_string();
    let mut gen_key: Option<PathBuf> = None;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || match it.next() {
            Some(v) => v.clone(),
            None => {
                eprintln!("missing value for {a}");
                std::process::exit(2);
            }
        };
        match a.as_str() {
            "--out" => out = PathBuf::from(val()),
            "--version" => version = val(),
            "--payload" => payload_path = Some(PathBuf::from(val())),
            "--inline-payload" => inline_payload = Some(PathBuf::from(val())),
            "--item" => {
                let spec = val();
                match parse_item(&spec, &cwd) {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(2);
                    }
                }
            }
            "--secret" => secret_hex = val(),
            "--product" => product = val(),
            "--notes" => notes = Some(val()),
            "--released" => released = val(),
            "--gen-key" => gen_key = Some(PathBuf::from(val())),
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
    }

    // --gen-key mode: emit a fresh random secret + derived public key.
    if let Some(dir) = gen_key {
        let mut seed = [0u8; 32];
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            if f.read_exact(&mut seed).is_err() {
                eprintln!("cannot read /dev/urandom");
                std::process::exit(2);
            }
        } else {
            eprintln!("no entropy source (/dev/urandom) available; refusing weak keygen");
            std::process::exit(2);
        }
        let pubkey = signing_public_key(&seed);
        let secret_hex = hex_encode(&seed);
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("secret.hex"), format!("{secret_hex}\n")).unwrap_or_else(|e| {
            eprintln!("cannot write secret.hex: {e}");
            std::process::exit(2);
        });
        std::fs::write(dir.join("public.hex"), format!("{}\n", hex_encode(&pubkey)))
            .unwrap_or_else(|e| {
                eprintln!("cannot write public.hex: {e}");
                std::process::exit(2);
            });
        println!("keypair written to {}", dir.display());
        println!("public_key: {}", hex_encode(&pubkey));
        return;
    }

    if version.is_empty() {
        eprintln!("--version is required");
        std::process::exit(2);
    }
    if secret_hex.is_empty() {
        eprintln!("--secret is required (build-host secret; see workstreams/security/crypto-integrity.md)");
        std::process::exit(2);
    }
    if payload_path.is_none() && inline_payload.is_none() && items.is_empty() {
        eprintln!("--payload FILE (single) or --inline-payload/--item (release) is required");
        std::process::exit(2);
    }
    let secret: [u8; 32] = parse_hex64(&secret_hex).unwrap_or_else(|e| {
        eprintln!("bad --secret: {e}");
        std::process::exit(2);
    });

    let mut payloads: Vec<PayloadMeta> = Vec::new();
    let mut payload_bytes: Vec<u8> = Vec::new();

    // Inline payload (the release manifest for release-engine bundles) is
    // pinned as payloads[0] so the checker's inline-integrity gate holds.
    if let Some(p) = &inline_payload {
        payload_bytes = std::fs::read(p).unwrap_or_else(|e| {
            eprintln!("cannot read inline payload {}: {e}", p.display());
            std::process::exit(2);
        });
        payloads.push(PayloadMeta {
            name: p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "payload.json".to_string()),
            sha256: sha256_hex(&payload_bytes),
            size: payload_bytes.len() as u64,
            url: None,
        });
    }

    if let Some(p) = payload_path {
        // Legacy single-payload mode.
        let bytes = std::fs::read(&p).unwrap_or_else(|e| {
            eprintln!("cannot read payload {}: {e}", p.display());
            std::process::exit(2);
        });
        if payload_bytes.is_empty() {
            payload_bytes = bytes.clone();
        }
        payloads.push(PayloadMeta {
            name: p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "payload.bin".to_string()),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
            url: None,
        });
    } else {
        for item in &items {
            payloads.push(PayloadMeta {
                name: item.name.clone(),
                sha256: item.sha256.clone(),
                size: item.size,
                url: item.url.clone(),
            });
        }
    }

    let manifest = Manifest {
        schema: "oep-update-manifest/1".to_string(),
        product,
        version: version.clone(),
        released,
        notes,
        payloads,
    };

    let bundle = sign_bundle(&manifest, &payload_bytes, &secret);
    let json = serde_json::to_vec_pretty(&bundle).unwrap();
    std::fs::write(&out, &json).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", out.display());
        std::process::exit(2);
    });
    println!("wrote {}", out.display());
    println!("version: {version}");
    println!("public_key: {}", hex_encode(&signing_public_key(&secret)));
}

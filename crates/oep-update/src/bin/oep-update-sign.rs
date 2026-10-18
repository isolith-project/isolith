//! `oep-update-sign` -- admin-side bundle signer for the P10-M5-T01 update
//! checker (decision #11, ties to P11 signing). Produces the signed offline
//! bundle the protected checker consumes.
//!
//! Usage:
//!   oep-update-sign --out OUT.json --version X.Y.Z --payload FILE \
//!       --secret HEX64 [--product oep] [--notes "..."]
//!
//! The secret is a 64-hex-char Ed25519 secret key. OUT.json is the
//! `BundleFile` readable by oep-update-check (and by the acceptance suite).

use std::path::PathBuf;

use oep_update::signing::{sign_bundle, signing_public_key};
use oep_update::{sha256_hex, Manifest, PayloadMeta};

fn main() {
    let mut out = PathBuf::from("update.bundle.json");
    let mut version = String::new();
    let mut payload_path = PathBuf::new();
    let mut secret_hex = String::new();
    let mut product = "oep".to_string();
    let mut notes: Option<String> = None;
    let mut released = "2026-10-17".to_string();

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().expect("missing value");
        match a.as_str() {
            "--out" => out = PathBuf::from(val()),
            "--version" => version = val(),
            "--payload" => payload_path = PathBuf::from(val()),
            "--secret" => secret_hex = val(),
            "--product" => product = val(),
            "--notes" => notes = Some(val()),
            "--released" => released = val(),
            "--help" | "-h" => {
                println!("usage: oep-update-sign --out OUT.json --version X.Y.Z --payload FILE --secret HEX64 [--product oep] [--notes ...]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    if version.is_empty() || payload_path.as_os_str().is_empty() || secret_hex.is_empty() {
        eprintln!("--version, --payload and --secret are required");
        std::process::exit(2);
    }
    let secret: [u8; 32] = parse_hex64(&secret_hex).unwrap_or_else(|e| {
        eprintln!("bad --secret: {e}");
        std::process::exit(2);
    });

    let payload = std::fs::read(&payload_path).unwrap_or_else(|e| {
        eprintln!("cannot read payload {}: {e}", payload_path.display());
        std::process::exit(2);
    });

    let manifest = Manifest {
        schema: "oep-update-manifest/1".to_string(),
        product,
        version: version.clone(),
        released,
        notes,
        payloads: vec![PayloadMeta {
            name: payload_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "payload.bin".to_string()),
            sha256: sha256_hex(&payload),
            size: payload.len() as u64,
            url: None,
        }],
    };

    let bundle = sign_bundle(&manifest, &payload, &secret);
    let json = serde_json::to_vec_pretty(&bundle).unwrap();
    std::fs::write(&out, &json).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", out.display());
        std::process::exit(2);
    });
    println!("wrote {}", out.display());
    println!("version: {version}");
    println!("public_key: {}", hex_encode(&signing_public_key(&secret)));
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

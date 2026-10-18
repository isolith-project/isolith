//! Signing helpers for the P10-M5-T01 update checker (decision #11 tie-in with
//! P11 signing). The checker VERIFIES; admins / the P12 release engine SIGN.
//!
//! Bundle format (`BundleFile`): a single JSON file carrying the user-visible
//! manifest fields, the Ed25519 signature over the canonical JSON of those
//! fields, and the payload bytes base64-inline. One file = one offline signed
//! bundle an admin can drop on a mirror or a USB key.

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{Manifest, PayloadMeta};

/// A signed update bundle: manifest fields + Ed25519 signature + inline
/// payload. This is what a remote/mirror/admin places for the checker to
/// consume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleFile {
    pub schema: String,
    pub product: String,
    pub version: String,
    pub released: String,
    pub notes: Option<String>,
    pub payloads: Vec<PayloadMeta>,
    pub payload_bytes_b64: String,
    pub signature_b64: String,
}

/// Canonical bytes that are signed: the manifest fields in this crate's
/// serialization order. Both signer and verifier use the identical struct, so
/// the bytes match.
pub fn manifest_canonical_bytes(
    schema: &str,
    product: &str,
    version: &str,
    released: &str,
    notes: &Option<String>,
    payloads: &[PayloadMeta],
) -> Vec<u8> {
    let m = Manifest {
        schema: schema.to_string(),
        product: product.to_string(),
        version: version.to_string(),
        released: released.to_string(),
        notes: notes.clone(),
        payloads: payloads.to_vec(),
    };
    serde_json::to_vec(&m).expect("manifest serializes")
}

/// Derive the 32-byte Ed25519 public key from a 32-byte secret key.
pub fn signing_public_key(secret: &[u8; 32]) -> [u8; 32] {
    let key = SigningKey::from_bytes(secret);
    key.verifying_key().to_bytes()
}

/// Sign a bundle: manifest fields + inline payload bytes, Ed25519-signed.
pub fn sign_bundle(manifest: &Manifest, payload_bytes: &[u8], secret: &[u8; 32]) -> BundleFile {
    let key = SigningKey::from_bytes(secret);
    let canonical = manifest_canonical_bytes(
        &manifest.schema,
        &manifest.product,
        &manifest.version,
        &manifest.released,
        &manifest.notes,
        &manifest.payloads,
    );
    let sig: Signature = key.sign(&canonical);
    BundleFile {
        schema: manifest.schema.clone(),
        product: manifest.product.clone(),
        version: manifest.version.clone(),
        released: manifest.released.clone(),
        notes: manifest.notes.clone(),
        payloads: manifest.payloads.clone(),
        payload_bytes_b64: base64::engine::general_purpose::STANDARD.encode(payload_bytes),
        signature_b64: base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()),
    }
}

/// Verify a bundle's Ed25519 signature against the provided public key.
pub fn verify_bundle(bundle: &BundleFile, public_key: &[u8; 32]) -> Result<(), String> {
    let vk = VerifyingKey::from_bytes(public_key).map_err(|e| format!("bad public key: {e}"))?;
    let canonical = manifest_canonical_bytes(
        &bundle.schema,
        &bundle.product,
        &bundle.version,
        &bundle.released,
        &bundle.notes,
        &bundle.payloads,
    );
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&bundle.signature_b64)
        .map_err(|e| format!("bad signature encoding: {e}"))?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|e| format!("bad signature: {e}"))?;
    vk.verify(&canonical, &sig)
        .map_err(|_| "Ed25519 signature verification failed".to_string())
}

impl BundleFile {
    /// Confirm the inline payload bytes hash to the manifest's first payload
    /// sha256 (integrity before exposure - decision #11 + P11 content-hash
    /// doctrine).
    pub fn verify_inline_payload(&self) -> Result<(), String> {
        let Some(meta) = self.payloads.first() else {
            return Ok(()); // manifest with no payloads is not an update
        };
        let bytes = self.inline_payload_bytes();
        crate::verify_payload(meta, &bytes)
    }

    /// Decode the inline base64 payload.
    pub fn inline_payload_bytes(&self) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.payload_bytes_b64)
            .unwrap_or_default()
    }
}

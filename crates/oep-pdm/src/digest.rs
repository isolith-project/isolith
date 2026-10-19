//! Content digest helper for the PDM vault (P7-M2-T01).
//!
//! The local stub content-addresses blobs with a 64-bit FNV-1a hash rendered
//! as hex. This is a NON-crypto integrity hash documented as such: the real
//! P8 server (P8-M1-T02) will use SHA-256 over its content-addressable blob
//! store; the contract does not depend on the hash choice, only on the digest
//! being stable (equal content maps to an equal digest, which needs no
//! cryptographic strength for the local working-set stub).

/// Compute the content digest of `bytes` as a lowercase 16-char hex string
/// (64-bit FNV-1a, offset basis 0xcbf29ce484222325, prime 0x100000001b3).
pub fn content_digest(bytes: &[u8]) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    format!("{h:016x}")
}

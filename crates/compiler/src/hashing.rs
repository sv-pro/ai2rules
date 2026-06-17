//! Deterministic hashing for descriptors and manifests (PLAN.md E1.4).
//!
//! Descriptor and manifest identities are SHA-256 over their JSON-normalized
//! form. `serde_json` serializes struct fields in declaration order and
//! `Value::Object` keys in sorted order (no `preserve_order` feature), so the
//! byte input is stable across runs.

use harness_types::{Descriptor, DescriptorHash, ManifestHash, WorldManifest};
use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_encode(&digest)
}

/// Hash an action descriptor over its canonical JSON form.
pub fn hash_descriptor(descriptor: &Descriptor) -> DescriptorHash {
    DescriptorHash::new(sha256_hex(descriptor.canonical_input().as_bytes()))
}

/// Hash a manifest over its canonical JSON form (the world version identity).
pub fn hash_manifest(manifest: &WorldManifest) -> ManifestHash {
    let canonical = serde_json::to_string(manifest).unwrap_or_default();
    ManifestHash::new(sha256_hex(canonical.as_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        // FIPS 180-2 test vector for "abc".
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_of_empty_is_known_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}

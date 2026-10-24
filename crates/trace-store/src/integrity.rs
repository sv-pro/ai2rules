//! Keyed integrity for the approval log (finding #15).
//!
//! The approval store is the one file in this system whose contents *grant*
//! something. An append-only JSONL log that anybody with write access can extend
//! is not an approval record; it is a suggestion. A line carrying an HMAC the
//! forger cannot compute makes an appended grant detectable.
//!
//! HMAC-SHA256 is implemented here over `sha2` rather than pulled in as a crate,
//! because the offline crate set has `sha2` and not `hmac`. It is ~20 lines of
//! RFC 2104, and it is checked against the RFC 4231 test vectors below — a hand-
//! rolled MAC with no known-answer tests would be worse than no MAC, since it
//! would look like protection.

use sha2::{Digest, Sha256};

const BLOCK: usize = 64; // SHA-256 block size, per RFC 2104.

/// HMAC-SHA256 of `msg` under `key`.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    // A key longer than the block is replaced by its digest; shorter is zero-padded.
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= block[i];
        opad[i] ^= block[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

/// Lowercase hex of an HMAC-SHA256.
pub fn hmac_hex(key: &[u8], msg: &[u8]) -> String {
    hmac_sha256(key, msg)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Compare two MAC strings without an early return.
///
/// A `==` on the hex would leak, through timing, how many leading bytes an
/// attacker guessed right — turning forgery into 32 cheap searches. The cost of
/// not having that problem is four lines.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// RFC 4231 §4.2 — the canonical "Hi There" vector.
    #[test]
    fn rfc4231_case_1() {
        let key = [0x0bu8; 20];
        let mac = hmac_sha256(&key, b"Hi There");
        assert_eq!(
            hex(&mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    /// RFC 4231 §4.3 — key shorter than the block, ASCII message.
    #[test]
    fn rfc4231_case_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// RFC 4231 §4.6 — key LONGER than the block, exercising the digest branch.
    #[test]
    fn rfc4231_case_6_long_key() {
        let key = [0xaau8; 131];
        let mac = hmac_sha256(
            &key,
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex(&mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn a_different_key_gives_a_different_mac() {
        assert_ne!(hmac_hex(b"k1", b"msg"), hmac_hex(b"k2", b"msg"));
    }

    #[test]
    fn a_changed_message_gives_a_different_mac() {
        assert_ne!(hmac_hex(b"k", b"approved"), hmac_hex(b"k", b"approvfd"));
    }

    #[test]
    fn constant_time_eq_still_compares_correctly() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }
}

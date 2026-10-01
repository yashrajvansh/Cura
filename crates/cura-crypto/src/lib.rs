//! HMAC-SHA-256 and SHA-256, matching the Elixir orchestrator byte-for-byte.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub fn sign(payload: &[u8], secret: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify(payload: &[u8], signature_hex: &str, secret: &[u8]) -> bool {
    let Ok(expected) = hex::decode(signature_hex) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(payload);
    mac.verify_slice(&expected).is_ok()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_shape() {
        let s = sign(b"hello", b"secret");
        assert_eq!(s.len(), 64);
        assert!(s
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn verify_ok() {
        let s = sign(b"p", b"k");
        assert!(verify(b"p", &s, b"k"));
    }

    #[test]
    fn verify_rejects() {
        let s = sign(b"p", b"k");
        assert!(!verify(b"q", &s, b"k"));
    }

    #[test]
    fn rfc_4231_case_2() {
        assert_eq!(
            sign(b"what do ya want for nothing?", b"Jefe"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn sha256_empty() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}

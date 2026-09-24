use super::{ct_eq_str, sha256};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

pub fn verify_s256(verifier: &str, challenge: &str) -> bool {
    if !(43..=128).contains(&verifier.len()) {
        return false;
    }
    if !verifier.bytes().all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        )
    }) {
        return false;
    }
    let digest = sha256(verifier.as_bytes());
    let encoded = URL_SAFE_NO_PAD.encode(digest);
    ct_eq_str(&encoded, challenge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_known_s256_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let digest = sha256(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(digest);
        assert!(verify_s256(verifier, &challenge));
        assert!(!verify_s256(verifier, "not-the-challenge"));
        assert!(!verify_s256("short", &challenge));
    }
}

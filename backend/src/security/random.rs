use crate::error::AppResult;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

pub fn random_bytes(len: usize) -> AppResult<Vec<u8>> {
    let mut buf = vec![0u8; len];
    getrandom_fill(&mut buf)?;
    Ok(buf)
}

pub fn random_token() -> AppResult<String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes(32)?))
}

pub fn fill_bytes(buf: &mut [u8]) -> AppResult<()> {
    getrandom_fill(buf)
}

fn getrandom_fill(buf: &mut [u8]) -> AppResult<()> {
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, buf);
    Ok(())
}

const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn recovery_code() -> AppResult<String> {
    let bytes = random_bytes(20)?;
    let chars: String = bytes
        .iter()
        .map(|byte| RECOVERY_ALPHABET[(*byte as usize) % RECOVERY_ALPHABET.len()] as char)
        .collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &chars[0..4],
        &chars[4..8],
        &chars[8..12],
        &chars[12..16],
        &chars[16..20]
    ))
}

pub fn normalize_recovery_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

pub fn email_otp() -> AppResult<String> {
    let mut bytes = [0u8; 4];
    fill_bytes(&mut bytes)?;
    let value = u32::from_be_bytes(bytes) % 1_000_000;
    Ok(format!("{value:06}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_code_shape() {
        let code = recovery_code().unwrap();
        assert_eq!(code.len(), 24);
        assert_eq!(normalize_recovery_code(&code).len(), 20);
    }

    #[test]
    fn otp_is_six_digits() {
        let code = email_otp().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}

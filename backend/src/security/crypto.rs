use crate::error::{AppError, AppResult};
use crate::security::random::random_bytes;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct TotpKeyring {
    active: u16,
    keys: BTreeMap<u16, [u8; 32]>,
}

impl TotpKeyring {
    pub fn from_spec(spec: &str, active: u16) -> AppResult<Self> {
        let mut keys = BTreeMap::new();
        for part in spec.split(',').filter(|part| !part.trim().is_empty()) {
            let Some((version, material)) = part.trim().split_once(':') else {
                return Err(AppError::internal("TOTP key spec must be version:base64"));
            };
            let version: u16 = version
                .trim()
                .parse()
                .map_err(|_| AppError::internal("invalid TOTP key version"))?;
            let bytes = decode_base64_flexible(material)
                .map_err(|_| AppError::internal("TOTP key is not base64"))?;
            if bytes.len() != 32 {
                return Err(AppError::internal("TOTP key must be 32 bytes"));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            keys.insert(version, key);
        }
        if !keys.contains_key(&active) {
            return Err(AppError::internal("active TOTP key version is missing"));
        }
        Ok(Self { active, keys })
    }

    pub fn generate_dev() -> (Self, String) {
        let mut key = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
        let mut keys = BTreeMap::new();
        keys.insert(1, key);
        let spec = format!("1:{}", STANDARD.encode(key));
        (Self { active: 1, keys }, spec)
    }

    pub fn active_version(&self) -> u16 {
        self.active
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> AppResult<(u16, Vec<u8>, Vec<u8>)> {
        let key = self.keys.get(&self.active).expect("active key checked");
        let cipher = Aes256Gcm::new_from_slice(key).map_err(AppError::internal)?;
        let nonce_bytes = random_bytes(12)?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: b"knotree-totp-v1",
                },
            )
            .map_err(AppError::internal)?;
        Ok((self.active, nonce_bytes, ciphertext))
    }

    pub fn decrypt(&self, version: u16, nonce: &[u8], ciphertext: &[u8]) -> AppResult<Vec<u8>> {
        let key = self
            .keys
            .get(&version)
            .ok_or_else(|| AppError::internal("unknown TOTP key version"))?;
        if nonce.len() != 12 {
            return Err(AppError::internal("invalid TOTP nonce"));
        }
        let cipher = Aes256Gcm::new_from_slice(key).map_err(AppError::internal)?;
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: b"knotree-totp-v1",
                },
            )
            .map_err(|_| AppError::internal("TOTP secret could not be decrypted"))
    }
}

/// Accepts standard or URL-safe base64, missing padding, and spaces that
/// replaced `+` while the value passed through a form or env panel.
pub fn decode_base64_flexible(material: &str) -> Result<Vec<u8>, ()> {
    let cleaned: String = material
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .chars()
        .map(|c| if c == ' ' { '+' } else { c })
        .filter(|c| !matches!(c, '\n' | '\r' | '\t'))
        .collect();
    if let Ok(bytes) = STANDARD.decode(&cleaned) {
        return Ok(bytes);
    }
    let padded = match cleaned.len() % 4 {
        0 => cleaned.clone(),
        n => format!("{cleaned}{}", "=".repeat(4 - n)),
    };
    STANDARD
        .decode(&padded)
        .or_else(|_| URL_SAFE.decode(&padded))
        .or_else(|_| URL_SAFE_NO_PAD.decode(cleaned.trim_end_matches('=')))
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_roundtrip() {
        let (ring, _) = TotpKeyring::generate_dev();
        let (version, nonce, ciphertext) = ring.encrypt(b"secret-bytes").unwrap();
        let plain = ring.decrypt(version, &nonce, &ciphertext).unwrap();
        assert_eq!(plain, b"secret-bytes");
    }

    #[test]
    fn totp_spec_accepts_unpadded_and_space_mangled_base64() {
        let spec = format!("1:{}", STANDARD.encode([9u8; 32]));
        let unpadded = spec.trim_end_matches('=');
        TotpKeyring::from_spec(unpadded, 1).unwrap();
        let spaced = spec.replace('+', " ");
        TotpKeyring::from_spec(&spaced, 1).unwrap();
    }
}

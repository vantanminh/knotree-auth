use crate::error::{AppError, AppResult};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

#[derive(Clone, Debug)]
pub struct ArgonSettings {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl ArgonSettings {
    pub fn production_default() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 1,
        }
    }

    pub fn test_fast() -> Self {
        Self {
            memory_kib: 8_192,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn argon2(&self) -> AppResult<Argon2<'static>> {
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, None)
            .map_err(AppError::internal)?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

pub fn hash_password(password: &str, settings: &ArgonSettings) -> AppResult<String> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    let argon = settings.argon2()?;
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(AppError::internal)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    let settings = ArgonSettings::production_default();
    let Ok(argon) = settings.argon2() else {
        return false;
    };
    argon.verify_password(password.as_bytes(), &parsed).is_ok()
}

pub fn needs_rehash(stored: &str, settings: &ArgonSettings) -> bool {
    let marker = format!(
        "$m={},t={},p={}$",
        settings.memory_kib, settings.iterations, settings.parallelism
    );
    !stored.contains(&marker)
}

const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password1",
    "password123",
    "123456789",
    "1234567890",
    "qwerty123",
    "qwertyuiop",
    "iloveyou",
    "admin123",
    "letmein",
    "welcome",
    "knotree",
    "knotree123",
];

pub fn validate_password(password: &str, email: &str) -> AppResult<()> {
    if password.len() < 10 {
        return Err(AppError::Validation("Use at least 10 characters."));
    }
    if password.len() > 1024 {
        return Err(AppError::Validation("Password is too long."));
    }
    if password.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(
            "Password contains unsupported characters.",
        ));
    }
    let lower = password.to_lowercase();
    if COMMON_PASSWORDS.contains(&lower.as_str()) {
        return Err(AppError::Validation("Choose a less common password."));
    }
    if lower == email.to_lowercase() {
        return Err(AppError::Validation("Password cannot match your email."));
    }
    Ok(())
}

pub fn normalize_email(input: &str) -> AppResult<String> {
    let email = input.trim().to_lowercase();
    if email.len() < 3 || email.len() > 254 {
        return Err(AppError::Validation("Enter a valid email address."));
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::Validation("Enter a valid email address."));
    };
    if local.is_empty()
        || local.len() > 64
        || domain.len() < 3
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || email.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::Validation("Enter a valid email address."));
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
        || !domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    {
        return Err(AppError::Validation("Enter a valid email address."));
    }
    Ok(email)
}

pub fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "••••".into();
    };
    let mut chars = local.chars();
    let first = chars.next().unwrap_or('•');
    format!("{first}••••@{domain}")
}

pub fn validate_display_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation("Enter a name up to 80 characters."));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(
            "Name contains unsupported characters.",
        ));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let settings = ArgonSettings::test_fast();
        let hash = hash_password("correct horse battery", &settings).unwrap();
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("wrong horse battery", &hash));
        assert!(!needs_rehash(&hash, &settings));
    }

    #[test]
    fn email_normalization() {
        assert_eq!(
            normalize_email("  Ada@Example.COM ").unwrap(),
            "ada@example.com"
        );
        assert!(normalize_email("not-an-email").is_err());
    }
}

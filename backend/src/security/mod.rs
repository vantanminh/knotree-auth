pub mod crypto;
pub mod password;
pub mod pkce;
pub mod random;
pub mod rate_limit;
pub mod redirect;
pub mod ua;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

pub fn ct_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    bool::from(left.ct_eq(right))
}

pub fn ct_eq_str(left: &str, right: &str) -> bool {
    ct_eq(&sha256(left.as_bytes()), &sha256(right.as_bytes()))
}

use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use sha2::Sha256;
use thiserror::Error;

pub const ITERATIONS: u32 = 20_000;
pub const KEY_LEN: usize = 256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Verification failed")]
    VerificationFailed,
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let mut salt = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut salt);

    let mut derived = [0u8; KEY_LEN];
    pbkdf2::<HmacSha256>(password.as_bytes(), &salt, ITERATIONS, &mut derived);

    use base64::Engine;
    let hash = base64::engine::general_purpose::STANDARD.encode(derived);
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);

    Ok(format!("${}${}${}", ITERATIONS, salt_b64, hash))
}

pub fn verify_password(password: &str, stored: &str) -> Result<bool, PasswordError> {
    let parts: Vec<&str> = stored.split('$').filter(|s| !s.is_empty()).collect();
    if parts.len() != 3 {
        return Err(PasswordError::VerificationFailed);
    }

    let iterations: u32 = parts[0].parse().map_err(|_| PasswordError::VerificationFailed)?;
    use base64::Engine;
    let salt = base64::engine::general_purpose::STANDARD.decode(parts[1])
        .map_err(|_| PasswordError::VerificationFailed)?;
    let stored_hash = base64::engine::general_purpose::STANDARD.decode(parts[2])
        .map_err(|_| PasswordError::VerificationFailed)?;

    let mut derived = [0u8; KEY_LEN];
    pbkdf2::<HmacSha256>(password.as_bytes(), &salt, iterations, &mut derived);

    Ok(derived.as_slice() == stored_hash.as_slice())
}

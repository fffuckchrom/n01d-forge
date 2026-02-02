//! Encryption module for n01d-forge
//! 
//! Supports:
//! - LUKS/LUKS2 encryption (Linux)
//! - VeraCrypt-compatible encryption
//! - AES-256-GCM for file-level encryption

use serde::{Deserialize, Serialize};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, password_hash::SaltString};
use rand::RngCore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionType {
    None,
    Luks,
    Luks2,
    VeraCrypt,
    Aes256Gcm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub encryption_type: EncryptionType,
    pub cipher: String,
    pub key_size: u32,
    pub hash_algorithm: String,
    pub iterations: u32,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            encryption_type: EncryptionType::Luks2,
            cipher: "aes-xts-plain64".to_string(),
            key_size: 512,
            hash_algorithm: "sha512".to_string(),
            iterations: 100000,
        }
    }
}

/// Derive encryption key from password using Argon2id
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let argon2 = Argon2::default();
    let mut key = [0u8; 32];
    
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Key derivation failed: {}", e))?;
    
    Ok(key)
}

/// Generate cryptographically secure random bytes
pub fn generate_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes
}

/// Generate a random salt for key derivation
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// Encrypt data using AES-256-GCM
pub fn encrypt_aes256gcm(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Cipher init failed: {}", e))?;
    
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    
    Ok(result)
}

/// Decrypt data using AES-256-GCM
pub fn decrypt_aes256gcm(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if encrypted.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }
    
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Cipher init failed: {}", e))?;
    
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];
    
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))
}

/// LUKS header structure (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuksHeader {
    pub version: u16,
    pub cipher_name: String,
    pub cipher_mode: String,
    pub hash_spec: String,
    pub payload_offset: u64,
    pub key_bytes: u32,
    pub mk_digest: [u8; 20],
    pub mk_digest_salt: [u8; 32],
    pub mk_digest_iter: u32,
    pub uuid: String,
}

impl LuksHeader {
    pub fn new(cipher: &str, hash: &str, key_size: u32, iterations: u32) -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        
        Self {
            version: 2,
            cipher_name: "aes".to_string(),
            cipher_mode: cipher.to_string(),
            hash_spec: hash.to_string(),
            payload_offset: 4096,
            key_bytes: key_size / 8,
            mk_digest: [0u8; 20],
            mk_digest_salt: generate_salt(),
            mk_digest_iter: iterations,
            uuid,
        }
    }
}

/// VeraCrypt volume header (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeraCryptHeader {
    pub version: u16,
    pub required_program_version: u16,
    pub crc32: u32,
    pub volume_creation_time: u64,
    pub header_creation_time: u64,
    pub hidden_volume_size: u64,
    pub volume_size: u64,
    pub encrypted_area_start: u64,
    pub encrypted_area_length: u64,
    pub flags: u32,
    pub sector_size: u32,
    pub encryption_algorithm: u32,
    pub hash_algorithm: u32,
    pub master_key: [u8; 64],
    pub secondary_key: [u8; 64],
    pub salt: [u8; 64],
}

impl VeraCryptHeader {
    pub fn new(volume_size: u64, encryption_algo: u32, hash_algo: u32) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut master_key = [0u8; 64];
        let mut secondary_key = [0u8; 64];
        let mut salt = [0u8; 64];
        
        rand::thread_rng().fill_bytes(&mut master_key);
        rand::thread_rng().fill_bytes(&mut secondary_key);
        rand::thread_rng().fill_bytes(&mut salt);
        
        Self {
            version: 5,
            required_program_version: 0x10b,
            crc32: 0,
            volume_creation_time: now,
            header_creation_time: now,
            hidden_volume_size: 0,
            volume_size,
            encrypted_area_start: 131072, // 128KB offset
            encrypted_area_length: volume_size - 131072,
            flags: 0,
            sector_size: 512,
            encryption_algorithm: encryption_algo,
            hash_algorithm: hash_algo,
            master_key,
            secondary_key,
            salt,
        }
    }
}

/// Encryption algorithm IDs for VeraCrypt
pub mod veracrypt_algorithms {
    pub const AES: u32 = 1;
    pub const SERPENT: u32 = 2;
    pub const TWOFISH: u32 = 3;
    pub const CAMELLIA: u32 = 4;
    pub const KUZNYECHIK: u32 = 5;
    pub const AES_TWOFISH: u32 = 6;
    pub const AES_TWOFISH_SERPENT: u32 = 7;
    pub const SERPENT_AES: u32 = 8;
    pub const SERPENT_TWOFISH_AES: u32 = 9;
    pub const TWOFISH_SERPENT: u32 = 10;
}

/// Hash algorithm IDs for VeraCrypt
pub mod veracrypt_hashes {
    pub const SHA512: u32 = 1;
    pub const WHIRLPOOL: u32 = 2;
    pub const SHA256: u32 = 3;
    pub const BLAKE2S: u32 = 4;
    pub const STREEBOG: u32 = 5;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_derivation() {
        let password = "test_password";
        let salt = generate_salt();
        
        let key1 = derive_key(password, &salt).unwrap();
        let key2 = derive_key(password, &salt).unwrap();
        
        assert_eq!(key1, key2);
    }
    
    #[test]
    fn test_aes_encryption() {
        let key = [0u8; 32];
        let data = b"Hello, World!";
        
        let encrypted = encrypt_aes256gcm(data, &key).unwrap();
        let decrypted = decrypt_aes256gcm(&encrypted, &key).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }
}

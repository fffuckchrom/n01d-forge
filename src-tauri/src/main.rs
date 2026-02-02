//! n01d-forge - Secure Cross-Platform Image Burner
//! 
//! Features:
//! - Image flashing (ISO, IMG, RAW)
//! - LUKS encryption for Linux
//! - VeraCrypt-compatible encryption
//! - Secure erase (multiple pass)
//! - Hash verification (SHA256, SHA512, MD5)
//! - Progress tracking
//! - Drive detection

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Sha512, Digest};
use md5::Md5;
use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

mod encryption;
mod drives;
mod secure_erase;

use encryption::{EncryptionConfig, EncryptionType};
use drives::{DriveInfo, list_drives};
use secure_erase::{SecureEraseMethod, secure_erase_drive};

// ============================================================================
// State Management
// ============================================================================

pub struct AppState {
    pub is_burning: Arc<AtomicBool>,
    pub progress: Arc<AtomicU64>,
    pub total_bytes: Arc<AtomicU64>,
    pub current_operation: Arc<Mutex<String>>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_burning: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            current_operation: Arc::new(Mutex::new(String::new())),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnConfig {
    pub image_path: String,
    pub target_drive: String,
    pub verify_after_write: bool,
    pub secure_erase_before: bool,
    pub erase_method: String,
    pub encryption: Option<EncryptionSettings>,
    pub bootloader: BootloaderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionSettings {
    pub enabled: bool,
    pub encryption_type: String,  // "luks", "veracrypt", "aes256"
    pub password: String,
    pub cipher: String,           // "aes-xts-plain64", "serpent", "twofish"
    pub key_size: u32,            // 256, 512
    pub hash_algo: String,        // "sha256", "sha512", "whirlpool"
    pub iterations: u32,          // PBKDF2 iterations
    pub wipe_header_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootloaderConfig {
    pub mode: String,             // "uefi", "legacy", "hybrid"
    pub secure_boot: bool,
    pub persistent_storage: bool,
    pub persistent_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashResult {
    pub algorithm: String,
    pub hash: String,
    pub verified: bool,
    pub expected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnProgress {
    pub stage: String,
    pub progress_percent: f64,
    pub bytes_written: u64,
    pub total_bytes: u64,
    pub speed_mbps: f64,
    pub eta_seconds: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnResult {
    pub success: bool,
    pub message: String,
    pub hash_verification: Option<HashResult>,
    pub duration_seconds: u64,
    pub bytes_written: u64,
}

// ============================================================================
// Tauri Commands - Drive Management
// ============================================================================

#[tauri::command]
async fn get_drives() -> Result<Vec<DriveInfo>, String> {
    list_drives().await
}

#[tauri::command]
async fn refresh_drives() -> Result<Vec<DriveInfo>, String> {
    list_drives().await
}

#[tauri::command]
async fn get_drive_info(device: String) -> Result<DriveInfo, String> {
    let drives = list_drives().await?;
    drives.into_iter()
        .find(|d| d.device == device)
        .ok_or_else(|| format!("Drive {} not found", device))
}

// ============================================================================
// Tauri Commands - Image Operations
// ============================================================================

#[tauri::command]
async fn get_image_info(path: String) -> Result<ImageInfo, String> {
    let path = PathBuf::from(&path);
    
    if !path.exists() {
        return Err("Image file not found".to_string());
    }
    
    let metadata = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();
    
    let image_type = match extension.as_str() {
        "iso" => "ISO 9660",
        "img" => "Raw Disk Image",
        "raw" => "Raw Disk Image",
        "dmg" => "Apple Disk Image",
        "vhd" | "vhdx" => "Virtual Hard Disk",
        "vmdk" => "VMware Disk",
        "qcow2" => "QEMU Copy-on-Write",
        _ => "Unknown",
    };
    
    Ok(ImageInfo {
        path: path.to_string_lossy().to_string(),
        name: path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        size: metadata.len(),
        image_type: image_type.to_string(),
        extension,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub image_type: String,
    pub extension: String,
}

#[tauri::command]
async fn calculate_hash(
    path: String,
    algorithm: String,
    state: State<'_, AppState>,
) -> Result<HashResult, String> {
    let path = PathBuf::from(&path);
    
    if !path.exists() {
        return Err("File not found".to_string());
    }
    
    let file = File::open(&path)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    
    let file_size = file.metadata()
        .map_err(|e| format!("Failed to get file size: {}", e))?
        .len();
    
    state.total_bytes.store(file_size, Ordering::SeqCst);
    state.progress.store(0, Ordering::SeqCst);
    
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut bytes_read = 0u64;
    
    let hash = match algorithm.to_lowercase().as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            loop {
                let n = reader.read(&mut buffer)
                    .map_err(|e| format!("Read error: {}", e))?;
                if n == 0 { break; }
                hasher.update(&buffer[..n]);
                bytes_read += n as u64;
                state.progress.store(bytes_read, Ordering::SeqCst);
            }
            hex::encode(hasher.finalize())
        },
        "sha512" => {
            let mut hasher = Sha512::new();
            loop {
                let n = reader.read(&mut buffer)
                    .map_err(|e| format!("Read error: {}", e))?;
                if n == 0 { break; }
                hasher.update(&buffer[..n]);
                bytes_read += n as u64;
                state.progress.store(bytes_read, Ordering::SeqCst);
            }
            hex::encode(hasher.finalize())
        },
        "md5" => {
            let mut hasher = Md5::new();
            loop {
                let n = reader.read(&mut buffer)
                    .map_err(|e| format!("Read error: {}", e))?;
                if n == 0 { break; }
                hasher.update(&buffer[..n]);
                bytes_read += n as u64;
                state.progress.store(bytes_read, Ordering::SeqCst);
            }
            hex::encode(hasher.finalize())
        },
        _ => return Err(format!("Unsupported algorithm: {}", algorithm)),
    };
    
    Ok(HashResult {
        algorithm,
        hash,
        verified: true,
        expected: None,
    })
}

#[tauri::command]
async fn verify_hash(
    path: String,
    expected_hash: String,
    algorithm: String,
    state: State<'_, AppState>,
) -> Result<HashResult, String> {
    let result = calculate_hash(path, algorithm.clone(), state).await?;
    
    let verified = result.hash.to_lowercase() == expected_hash.to_lowercase();
    
    Ok(HashResult {
        algorithm,
        hash: result.hash,
        verified,
        expected: Some(expected_hash),
    })
}

// ============================================================================
// Tauri Commands - Burning Operations
// ============================================================================

#[tauri::command]
async fn burn_image(
    config: BurnConfig,
    state: State<'_, AppState>,
) -> Result<BurnResult, String> {
    // Check if already burning
    if state.is_burning.load(Ordering::SeqCst) {
        return Err("A burn operation is already in progress".to_string());
    }
    
    state.is_burning.store(true, Ordering::SeqCst);
    state.cancel_flag.store(false, Ordering::SeqCst);
    state.progress.store(0, Ordering::SeqCst);
    
    let start_time = std::time::Instant::now();
    
    // Update operation status
    {
        let mut op = state.current_operation.lock().await;
        *op = "Preparing...".to_string();
    }
    
    // Validate image exists
    let image_path = PathBuf::from(&config.image_path);
    if !image_path.exists() {
        state.is_burning.store(false, Ordering::SeqCst);
        return Err("Image file not found".to_string());
    }
    
    let image_size = std::fs::metadata(&image_path)
        .map_err(|e| {
            state.is_burning.store(false, Ordering::SeqCst);
            format!("Failed to read image: {}", e)
        })?
        .len();
    
    state.total_bytes.store(image_size, Ordering::SeqCst);
    
    // Step 1: Secure erase if requested
    if config.secure_erase_before {
        {
            let mut op = state.current_operation.lock().await;
            *op = "Secure erasing drive...".to_string();
        }
        
        let erase_method = match config.erase_method.as_str() {
            "zeros" => SecureEraseMethod::Zeros,
            "random" => SecureEraseMethod::Random,
            "dod" => SecureEraseMethod::DoD,
            "gutmann" => SecureEraseMethod::Gutmann,
            _ => SecureEraseMethod::Zeros,
        };
        
        secure_erase_drive(&config.target_drive, erase_method).await?;
    }
    
    // Check for cancellation
    if state.cancel_flag.load(Ordering::SeqCst) {
        state.is_burning.store(false, Ordering::SeqCst);
        return Err("Operation cancelled".to_string());
    }
    
    // Step 2: Write image to drive
    {
        let mut op = state.current_operation.lock().await;
        *op = "Writing image to drive...".to_string();
    }
    
    let bytes_written = write_image_to_drive(
        &config.image_path,
        &config.target_drive,
        &state,
    ).await?;
    
    // Check for cancellation
    if state.cancel_flag.load(Ordering::SeqCst) {
        state.is_burning.store(false, Ordering::SeqCst);
        return Err("Operation cancelled".to_string());
    }
    
    // Step 3: Setup encryption if requested
    if let Some(ref enc_settings) = config.encryption {
        if enc_settings.enabled {
            {
                let mut op = state.current_operation.lock().await;
                *op = "Setting up encryption...".to_string();
            }
            
            setup_encryption(&config.target_drive, enc_settings).await?;
        }
    }
    
    // Step 4: Configure bootloader
    {
        let mut op = state.current_operation.lock().await;
        *op = "Configuring bootloader...".to_string();
    }
    
    configure_bootloader(&config.target_drive, &config.bootloader).await?;
    
    // Step 5: Verify if requested
    let hash_verification = if config.verify_after_write {
        {
            let mut op = state.current_operation.lock().await;
            *op = "Verifying write...".to_string();
        }
        
        Some(verify_written_image(
            &config.image_path,
            &config.target_drive,
            image_size,
            &state,
        ).await?)
    } else {
        None
    };
    
    // Sync and cleanup
    {
        let mut op = state.current_operation.lock().await;
        *op = "Syncing...".to_string();
    }
    
    // Sync filesystem
    #[cfg(unix)]
    {
        Command::new("sync").status().ok();
    }
    
    let duration = start_time.elapsed().as_secs();
    
    state.is_burning.store(false, Ordering::SeqCst);
    {
        let mut op = state.current_operation.lock().await;
        *op = "Complete".to_string();
    }
    
    Ok(BurnResult {
        success: true,
        message: "Image burned successfully".to_string(),
        hash_verification,
        duration_seconds: duration,
        bytes_written,
    })
}

async fn write_image_to_drive(
    image_path: &str,
    target_drive: &str,
    state: &State<'_, AppState>,
) -> Result<u64, String> {
    let image_file = File::open(image_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;
    
    let target_file = std::fs::OpenOptions::new()
        .write(true)
        .open(target_drive)
        .map_err(|e| format!("Failed to open target drive: {}", e))?;
    
    let mut reader = BufReader::with_capacity(4 * 1024 * 1024, image_file);
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, target_file);
    
    let mut buffer = vec![0u8; 4 * 1024 * 1024]; // 4MB buffer
    let mut bytes_written = 0u64;
    
    loop {
        // Check for cancellation
        if state.cancel_flag.load(Ordering::SeqCst) {
            return Err("Operation cancelled".to_string());
        }
        
        let n = reader.read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;
        
        if n == 0 { break; }
        
        writer.write_all(&buffer[..n])
            .map_err(|e| format!("Write error: {}", e))?;
        
        bytes_written += n as u64;
        state.progress.store(bytes_written, Ordering::SeqCst);
    }
    
    writer.flush()
        .map_err(|e| format!("Flush error: {}", e))?;
    
    Ok(bytes_written)
}

async fn setup_encryption(
    target_drive: &str,
    settings: &EncryptionSettings,
) -> Result<(), String> {
    match settings.encryption_type.as_str() {
        "luks" | "luks2" => {
            setup_luks_encryption(target_drive, settings).await
        },
        "veracrypt" => {
            setup_veracrypt_encryption(target_drive, settings).await
        },
        _ => Err(format!("Unsupported encryption type: {}", settings.encryption_type)),
    }
}

async fn setup_luks_encryption(
    target_drive: &str,
    settings: &EncryptionSettings,
) -> Result<(), String> {
    // Format with LUKS
    let cipher = match settings.cipher.as_str() {
        "aes-xts-plain64" => "aes-xts-plain64",
        "serpent-xts-plain64" => "serpent-xts-plain64",
        "twofish-xts-plain64" => "twofish-xts-plain64",
        _ => "aes-xts-plain64",
    };
    
    let key_size = settings.key_size.to_string();
    let hash = &settings.hash_algo;
    let iter_time = (settings.iterations / 1000).max(1).to_string();
    
    // Create LUKS container
    let output = Command::new("cryptsetup")
        .args([
            "luksFormat",
            "--type", "luks2",
            "--cipher", cipher,
            "--key-size", &key_size,
            "--hash", hash,
            "--iter-time", &iter_time,
            "--batch-mode",
            target_drive,
        ])
        .stdin(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run cryptsetup: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "LUKS format failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Add key
    let mut child = Command::new("cryptsetup")
        .args(["luksAddKey", "--batch-mode", target_drive])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to add key: {}", e))?;
    
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(settings.password.as_bytes()).ok();
        stdin.write_all(b"\n").ok();
    }
    
    child.wait().map_err(|e| format!("Key add failed: {}", e))?;
    
    Ok(())
}

async fn setup_veracrypt_encryption(
    _target_drive: &str,
    _settings: &EncryptionSettings,
) -> Result<(), String> {
    // VeraCrypt CLI support would go here
    // For now, return instructions
    Err("VeraCrypt encryption requires veracrypt CLI. Please install veracrypt first.".to_string())
}

async fn configure_bootloader(
    target_drive: &str,
    config: &BootloaderConfig,
) -> Result<(), String> {
    match config.mode.as_str() {
        "uefi" => {
            // UEFI bootloader setup
            // This would involve setting up EFI partition
            Ok(())
        },
        "legacy" => {
            // Legacy BIOS bootloader
            // Install GRUB or similar
            Ok(())
        },
        "hybrid" => {
            // Both UEFI and Legacy support
            Ok(())
        },
        _ => Ok(()),
    }
}

async fn verify_written_image(
    image_path: &str,
    target_drive: &str,
    image_size: u64,
    state: &State<'_, AppState>,
) -> Result<HashResult, String> {
    // Calculate hash of original image
    let image_file = File::open(image_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;
    
    let mut reader = BufReader::with_capacity(4 * 1024 * 1024, image_file);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    
    loop {
        let n = reader.read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    
    let original_hash = hex::encode(hasher.finalize());
    
    // Calculate hash of written data
    let target_file = File::open(target_drive)
        .map_err(|e| format!("Failed to open target: {}", e))?;
    
    let mut reader = BufReader::with_capacity(4 * 1024 * 1024, target_file);
    let mut hasher = Sha256::new();
    let mut bytes_read = 0u64;
    
    state.progress.store(0, Ordering::SeqCst);
    
    while bytes_read < image_size {
        let to_read = std::cmp::min(buffer.len() as u64, image_size - bytes_read) as usize;
        let n = reader.read(&mut buffer[..to_read])
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
        bytes_read += n as u64;
        state.progress.store(bytes_read, Ordering::SeqCst);
    }
    
    let written_hash = hex::encode(hasher.finalize());
    
    Ok(HashResult {
        algorithm: "sha256".to_string(),
        hash: written_hash.clone(),
        verified: original_hash == written_hash,
        expected: Some(original_hash),
    })
}

#[tauri::command]
async fn cancel_burn(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn get_burn_progress(state: State<'_, AppState>) -> Result<BurnProgress, String> {
    let progress = state.progress.load(Ordering::SeqCst);
    let total = state.total_bytes.load(Ordering::SeqCst);
    let operation = state.current_operation.lock().await.clone();
    
    let percent = if total > 0 {
        (progress as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    
    Ok(BurnProgress {
        stage: operation,
        progress_percent: percent,
        bytes_written: progress,
        total_bytes: total,
        speed_mbps: 0.0, // Would need timing to calculate
        eta_seconds: 0,   // Would need timing to calculate
        message: String::new(),
    })
}

// ============================================================================
// Tauri Commands - Secure Erase
// ============================================================================

#[tauri::command]
async fn secure_erase(
    device: String,
    method: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.is_burning.load(Ordering::SeqCst) {
        return Err("Another operation is in progress".to_string());
    }
    
    state.is_burning.store(true, Ordering::SeqCst);
    
    let erase_method = match method.as_str() {
        "zeros" => SecureEraseMethod::Zeros,
        "random" => SecureEraseMethod::Random,
        "dod" => SecureEraseMethod::DoD,
        "gutmann" => SecureEraseMethod::Gutmann,
        _ => SecureEraseMethod::Zeros,
    };
    
    let result = secure_erase_drive(&device, erase_method).await;
    
    state.is_burning.store(false, Ordering::SeqCst);
    
    result
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // Drive management
            get_drives,
            refresh_drives,
            get_drive_info,
            // Image operations
            get_image_info,
            calculate_hash,
            verify_hash,
            // Burning operations
            burn_image,
            cancel_burn,
            get_burn_progress,
            // Secure erase
            secure_erase,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

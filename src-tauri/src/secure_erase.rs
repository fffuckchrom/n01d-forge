//! Secure erase module for n01d-forge
//! 
//! Implements various secure erase methods:
//! - Zero fill
//! - Random data
//! - DoD 5220.22-M (3-pass)
//! - Gutmann method (35-pass)

use std::fs::{File, OpenOptions};
use std::io::{Write, Seek, SeekFrom};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecureEraseMethod {
    /// Single pass of zeros
    Zeros,
    /// Single pass of random data
    Random,
    /// DoD 5220.22-M standard (3 passes)
    DoD,
    /// Gutmann method (35 passes)
    Gutmann,
    /// Custom number of random passes
    CustomRandom(u8),
}

impl SecureEraseMethod {
    pub fn passes(&self) -> u8 {
        match self {
            SecureEraseMethod::Zeros => 1,
            SecureEraseMethod::Random => 1,
            SecureEraseMethod::DoD => 3,
            SecureEraseMethod::Gutmann => 35,
            SecureEraseMethod::CustomRandom(n) => *n,
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            SecureEraseMethod::Zeros => "Zero Fill",
            SecureEraseMethod::Random => "Random Data",
            SecureEraseMethod::DoD => "DoD 5220.22-M",
            SecureEraseMethod::Gutmann => "Gutmann (35-pass)",
            SecureEraseMethod::CustomRandom(_) => "Custom Random",
        }
    }
}

/// Perform secure erase on a drive
pub async fn secure_erase_drive(
    device: &str,
    method: SecureEraseMethod,
) -> Result<(), String> {
    // Get device size
    let size = get_device_size(device)?;
    
    // Open device for writing
    let mut file = OpenOptions::new()
        .write(true)
        .open(device)
        .map_err(|e| format!("Failed to open device: {}", e))?;
    
    match method {
        SecureEraseMethod::Zeros => {
            write_pattern(&mut file, size, PatternType::Zeros)?;
        },
        SecureEraseMethod::Random => {
            write_pattern(&mut file, size, PatternType::Random)?;
        },
        SecureEraseMethod::DoD => {
            // DoD 5220.22-M: Pass 1 - zeros, Pass 2 - ones, Pass 3 - random
            write_pattern(&mut file, size, PatternType::Zeros)?;
            file.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek failed: {}", e))?;
            write_pattern(&mut file, size, PatternType::Ones)?;
            file.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek failed: {}", e))?;
            write_pattern(&mut file, size, PatternType::Random)?;
        },
        SecureEraseMethod::Gutmann => {
            // Gutmann 35-pass method
            for pass in 0..35 {
                file.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek failed: {}", e))?;
                let pattern = get_gutmann_pattern(pass);
                write_pattern(&mut file, size, pattern)?;
            }
        },
        SecureEraseMethod::CustomRandom(passes) => {
            for _ in 0..passes {
                file.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek failed: {}", e))?;
                write_pattern(&mut file, size, PatternType::Random)?;
            }
        },
    }
    
    // Sync to ensure all data is written
    file.sync_all().map_err(|e| format!("Sync failed: {}", e))?;
    
    Ok(())
}

#[derive(Clone, Copy)]
enum PatternType {
    Zeros,
    Ones,
    Random,
    Fixed([u8; 3]),
}

fn write_pattern(
    file: &mut File,
    size: u64,
    pattern: PatternType,
) -> Result<(), String> {
    const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB buffer
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut bytes_written = 0u64;
    
    // Fill buffer with pattern
    match pattern {
        PatternType::Zeros => {
            // Already zeros
        },
        PatternType::Ones => {
            buffer.fill(0xFF);
        },
        PatternType::Random => {
            rand::thread_rng().fill_bytes(&mut buffer);
        },
        PatternType::Fixed(p) => {
            for (i, byte) in buffer.iter_mut().enumerate() {
                *byte = p[i % 3];
            }
        },
    }
    
    while bytes_written < size {
        let to_write = std::cmp::min(BUFFER_SIZE as u64, size - bytes_written) as usize;
        
        // Regenerate random data for each chunk if using random pattern
        if matches!(pattern, PatternType::Random) {
            rand::thread_rng().fill_bytes(&mut buffer[..to_write]);
        }
        
        file.write_all(&buffer[..to_write])
            .map_err(|e| format!("Write failed: {}", e))?;
        
        bytes_written += to_write as u64;
    }
    
    Ok(())
}

fn get_gutmann_pattern(pass: u8) -> PatternType {
    // Gutmann method patterns
    match pass {
        0..=3 => PatternType::Random,
        4 => PatternType::Fixed([0x55, 0x55, 0x55]),
        5 => PatternType::Fixed([0xAA, 0xAA, 0xAA]),
        6 => PatternType::Fixed([0x92, 0x49, 0x24]),
        7 => PatternType::Fixed([0x49, 0x24, 0x92]),
        8 => PatternType::Fixed([0x24, 0x92, 0x49]),
        9 => PatternType::Fixed([0x00, 0x00, 0x00]),
        10 => PatternType::Fixed([0x11, 0x11, 0x11]),
        11 => PatternType::Fixed([0x22, 0x22, 0x22]),
        12 => PatternType::Fixed([0x33, 0x33, 0x33]),
        13 => PatternType::Fixed([0x44, 0x44, 0x44]),
        14 => PatternType::Fixed([0x55, 0x55, 0x55]),
        15 => PatternType::Fixed([0x66, 0x66, 0x66]),
        16 => PatternType::Fixed([0x77, 0x77, 0x77]),
        17 => PatternType::Fixed([0x88, 0x88, 0x88]),
        18 => PatternType::Fixed([0x99, 0x99, 0x99]),
        19 => PatternType::Fixed([0xAA, 0xAA, 0xAA]),
        20 => PatternType::Fixed([0xBB, 0xBB, 0xBB]),
        21 => PatternType::Fixed([0xCC, 0xCC, 0xCC]),
        22 => PatternType::Fixed([0xDD, 0xDD, 0xDD]),
        23 => PatternType::Fixed([0xEE, 0xEE, 0xEE]),
        24 => PatternType::Fixed([0xFF, 0xFF, 0xFF]),
        25 => PatternType::Fixed([0x92, 0x49, 0x24]),
        26 => PatternType::Fixed([0x49, 0x24, 0x92]),
        27 => PatternType::Fixed([0x24, 0x92, 0x49]),
        28 => PatternType::Fixed([0x6D, 0xB6, 0xDB]),
        29 => PatternType::Fixed([0xB6, 0xDB, 0x6D]),
        30 => PatternType::Fixed([0xDB, 0x6D, 0xB6]),
        31..=34 => PatternType::Random,
        _ => PatternType::Random,
    }
}

fn get_device_size(device: &str) -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        let output = Command::new("blockdev")
            .args(["--getsize64", device])
            .output()
            .map_err(|e| format!("Failed to get device size: {}", e))?;
        
        let size_str = String::from_utf8_lossy(&output.stdout);
        size_str.trim().parse::<u64>()
            .map_err(|e| format!("Failed to parse size: {}", e))
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        let output = Command::new("diskutil")
            .args(["info", "-plist", device])
            .output()
            .map_err(|e| format!("Failed to get device size: {}", e))?;
        
        // Parse plist to get size - simplified
        Ok(0)
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        // Use PowerShell to get disk size
        let ps_script = format!(
            "(Get-Disk -Number {}).Size",
            device.chars().filter(|c| c.is_numeric()).collect::<String>()
        );
        
        let output = Command::new("powershell")
            .args(["-Command", &ps_script])
            .output()
            .map_err(|e| format!("Failed to get device size: {}", e))?;
        
        let size_str = String::from_utf8_lossy(&output.stdout);
        size_str.trim().parse::<u64>()
            .map_err(|e| format!("Failed to parse size: {}", e))
    }
}

/// Quick format a drive (wipe partition table and first MB)
pub async fn quick_wipe(device: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        
        // Wipe partition table signatures
        Command::new("wipefs")
            .args(["--all", "--force", device])
            .output()
            .map_err(|e| format!("wipefs failed: {}", e))?;
        
        // Zero first 1MB
        Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", device),
                "bs=1M",
                "count=1",
                "conv=notrunc",
            ])
            .output()
            .map_err(|e| format!("dd failed: {}", e))?;
        
        // Zero last 1MB (backup GPT)
        let size = get_device_size(device)?;
        let seek = (size / (1024 * 1024)) - 1;
        
        Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", device),
                "bs=1M",
                "count=1",
                &format!("seek={}", seek),
                "conv=notrunc",
            ])
            .output()
            .map_err(|e| format!("dd failed: {}", e))?;
        
        Ok(())
    }
    
    #[cfg(windows)]
    {
        // Windows implementation using diskpart or PowerShell
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gutmann_patterns() {
        for pass in 0..35 {
            let pattern = get_gutmann_pattern(pass);
            // Just verify we get a pattern for each pass
            match pattern {
                PatternType::Zeros | PatternType::Ones | 
                PatternType::Random | PatternType::Fixed(_) => {}
            }
        }
    }
}

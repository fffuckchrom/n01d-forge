//! Drive detection and management module for n01d-forge

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub device: String,
    pub name: String,
    pub size: u64,
    pub size_human: String,
    pub model: String,
    pub vendor: String,
    pub serial: String,
    pub is_removable: bool,
    pub is_usb: bool,
    pub mount_points: Vec<String>,
    pub partitions: Vec<PartitionInfo>,
    pub bus_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub device: String,
    pub label: String,
    pub filesystem: String,
    pub size: u64,
    pub mount_point: Option<String>,
}

/// List all available drives
pub async fn list_drives() -> Result<Vec<DriveInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        list_drives_linux().await
    }
    
    #[cfg(target_os = "windows")]
    {
        list_drives_windows().await
    }
    
    #[cfg(target_os = "macos")]
    {
        list_drives_macos().await
    }
}

#[cfg(target_os = "linux")]
async fn list_drives_linux() -> Result<Vec<DriveInfo>, String> {
    let output = Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,MODEL,VENDOR,SERIAL,RM,TRAN,LABEL,FSTYPE"])
        .output()
        .map_err(|e| format!("Failed to run lsblk: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let lsblk: LsblkOutput = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse lsblk output: {}", e))?;
    
    let mut drives = Vec::new();
    
    for device in lsblk.blockdevices {
        // Skip non-disk devices
        if device.device_type != "disk" {
            continue;
        }
        
        // Skip if it looks like a system drive
        let is_system = device.children.as_ref().map_or(false, |children| {
            children.iter().any(|p| {
                p.mountpoint.as_ref().map_or(false, |mp| {
                    mp == "/" || mp == "/boot" || mp == "/home" || mp.starts_with("/boot")
                })
            })
        });
        
        if is_system {
            continue;
        }
        
        let is_removable = device.rm.unwrap_or(false);
        let is_usb = device.tran.as_ref().map_or(false, |t| t == "usb");
        
        // Collect mount points
        let mut mount_points = Vec::new();
        let mut partitions = Vec::new();
        
        if let Some(children) = &device.children {
            for child in children {
                if let Some(mp) = &child.mountpoint {
                    if !mp.is_empty() {
                        mount_points.push(mp.clone());
                    }
                }
                
                partitions.push(PartitionInfo {
                    device: format!("/dev/{}", child.name),
                    label: child.label.clone().unwrap_or_default(),
                    filesystem: child.fstype.clone().unwrap_or_default(),
                    size: child.size.unwrap_or(0),
                    mount_point: child.mountpoint.clone(),
                });
            }
        }
        
        let size = device.size.unwrap_or(0);
        
        drives.push(DriveInfo {
            device: format!("/dev/{}", device.name),
            name: device.name.clone(),
            size,
            size_human: format_size(size),
            model: device.model.clone().unwrap_or_default().trim().to_string(),
            vendor: device.vendor.clone().unwrap_or_default().trim().to_string(),
            serial: device.serial.clone().unwrap_or_default(),
            is_removable,
            is_usb,
            mount_points,
            partitions,
            bus_type: device.tran.clone().unwrap_or_else(|| "unknown".to_string()),
        });
    }
    
    Ok(drives)
}

#[cfg(target_os = "windows")]
async fn list_drives_windows() -> Result<Vec<DriveInfo>, String> {
    use std::process::Command;
    
    // Use PowerShell to get drive information
    let ps_script = r#"
        Get-Disk | Where-Object { $_.BusType -eq 'USB' -or $_.IsSystem -eq $false } | ForEach-Object {
            $disk = $_
            $partitions = Get-Partition -DiskNumber $_.Number -ErrorAction SilentlyContinue
            [PSCustomObject]@{
                Number = $_.Number
                Size = $_.Size
                Model = $_.Model
                SerialNumber = $_.SerialNumber
                BusType = $_.BusType
                IsRemovable = ($_.BusType -eq 'USB')
                Partitions = $partitions | ForEach-Object {
                    [PSCustomObject]@{
                        DriveLetter = $_.DriveLetter
                        Size = $_.Size
                        Type = $_.Type
                    }
                }
            }
        } | ConvertTo-Json -Depth 3
    "#;
    
    let output = Command::new("powershell")
        .args(["-Command", ps_script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "PowerShell failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Parse JSON output and convert to DriveInfo
    let json_str = String::from_utf8_lossy(&output.stdout);
    
    // For now, return a placeholder - full Windows implementation would parse the JSON
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
async fn list_drives_macos() -> Result<Vec<DriveInfo>, String> {
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| format!("Failed to run diskutil: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "diskutil failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // For now, return a placeholder - full macOS implementation would parse the plist
    Ok(Vec::new())
}

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    name: String,
    size: Option<u64>,
    #[serde(rename = "type")]
    device_type: String,
    mountpoint: Option<String>,
    model: Option<String>,
    vendor: Option<String>,
    serial: Option<String>,
    rm: Option<bool>,
    tran: Option<String>,
    label: Option<String>,
    fstype: Option<String>,
    children: Option<Vec<LsblkDevice>>,
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    
    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Unmount all partitions on a drive
pub async fn unmount_drive(device: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("umount")
            .arg(device)
            .output()
            .map_err(|e| format!("Failed to unmount: {}", e))?;
        
        // It's okay if unmount fails (might not be mounted)
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("diskutil")
            .args(["unmountDisk", device])
            .output()
            .map_err(|e| format!("Failed to unmount: {}", e))?;
        Ok(())
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows doesn't need explicit unmount for raw disk access
        Ok(())
    }
}

/// Eject a drive safely
pub async fn eject_drive(device: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // First unmount
        unmount_drive(device).await?;
        
        // Then eject
        Command::new("eject")
            .arg(device)
            .output()
            .map_err(|e| format!("Failed to eject: {}", e))?;
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("diskutil")
            .args(["eject", device])
            .output()
            .map_err(|e| format!("Failed to eject: {}", e))?;
        Ok(())
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows eject through PowerShell
        let ps_script = format!(
            r#"
            $vol = Get-Volume -DriveLetter {} -ErrorAction SilentlyContinue
            if ($vol) {{
                $eject = New-Object -ComObject Shell.Application
                $eject.Namespace(17).ParseName("{}:\").InvokeVerb("Eject")
            }}
            "#,
            device, device
        );
        
        Command::new("powershell")
            .args(["-Command", &ps_script])
            .output()
            .map_err(|e| format!("Failed to eject: {}", e))?;
        Ok(())
    }
}

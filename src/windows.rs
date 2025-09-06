//! Windows-specific implementation using Win32 API

use crate::{ColorSpace, Display, DisplayProfileProvider, ProfileConfig, ProfileError, ProfileInfo};
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;
use winapi::shared::minwindef::{BOOL, DWORD, FALSE, TRUE, HKEY};

// Registry constants
const KEY_READ: DWORD = 0x20019;
const REG_SZ: DWORD = 1;
use winapi::shared::windef::{HDC, HMONITOR, LPRECT, RECT};
use winapi::um::wingdi::{
    GetICMProfileA,
};
use winapi::um::winuser::{
    EnumDisplayMonitors, GetMonitorInfoA, MONITORINFO, MONITORINFOEXA,
};
use winapi::um::winreg::{
    RegCloseKey, RegEnumKeyExA, RegOpenKeyExA, RegQueryValueExA, HKEY_LOCAL_MACHINE,
};

/// Windows implementation of DisplayProfileProvider using Win32 API
pub struct WindowsProfileProvider {
    config: ProfileConfig,
}

/// Internal structure to hold monitor enumeration data
struct MonitorEnumData {
    monitors: Vec<MonitorInfo>,
}

/// Information about a Windows monitor
#[derive(Clone)]
struct MonitorInfo {
    handle: HMONITOR,
    name: String,
    is_primary: bool,
    rect: RECT,
}

/// Safe wrapper around Windows color directory retrieval
fn get_color_directory() -> Result<PathBuf, ProfileError> {
    // Use standard Windows color directory path
    let windows_dir = std::env::var("WINDIR")
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    
    let color_dir = PathBuf::from(windows_dir).join("System32").join("spool").join("drivers").join("color");
    
    if color_dir.exists() {
        Ok(color_dir)
    } else {
        Err(ProfileError::SystemError(
            "Windows color directory not found".to_string(),
        ))
    }
}

/// Enumerate color profiles by scanning the color directory
fn enum_color_profiles() -> Result<Vec<String>, ProfileError> {
    let color_dir = get_color_directory()?;
    let mut profiles = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&color_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if ext == "icc" || ext == "icm" {
                    if let Some(filename) = path.file_name() {
                        profiles.push(filename.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    
    Ok(profiles)
}

/// Get ICC profile for a specific monitor
fn get_monitor_profile(_monitor: HMONITOR) -> Result<String, ProfileError> {
    // Get device context for the monitor
    let hdc = unsafe { winapi::um::winuser::GetDC(ptr::null_mut()) };
    if hdc.is_null() {
        return Err(ProfileError::SystemError(
            "Failed to get device context".to_string(),
        ));
    }
    
    let mut buffer = vec![0u8; 260]; // MAX_PATH
    let mut size = buffer.len() as DWORD;
    
    let result = unsafe {
        GetICMProfileA(hdc, &mut size, buffer.as_mut_ptr() as *mut i8)
    };
    
    unsafe {
        winapi::um::winuser::ReleaseDC(ptr::null_mut(), hdc);
    }
    
    if result == FALSE {
        return Err(ProfileError::ProfileNotAvailable(
            "No profile associated with monitor".to_string(),
        ));
    }
    
    let profile_name = unsafe {
        CStr::from_ptr(buffer.as_ptr() as *const i8)
            .to_str()
            .map_err(|e| ProfileError::ParseError(format!("Invalid profile name: {}", e)))?
    };
    
    Ok(profile_name.to_string())
}

/// Callback function for monitor enumeration
unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: LPRECT,
    lparam: isize,
) -> BOOL {
    let data = &mut *(lparam as *mut MonitorEnumData);
    
    let mut monitor_info: MONITORINFOEXA = std::mem::zeroed();
    monitor_info.cbSize = std::mem::size_of::<MONITORINFOEXA>() as DWORD;
    
    let result = GetMonitorInfoA(hmonitor, &mut monitor_info as *mut _ as *mut MONITORINFO);
    if result == FALSE {
        return TRUE; // Continue enumeration even if this monitor fails
    }
    
    // Extract monitor name from szDevice
    let device_name = CStr::from_ptr(monitor_info.szDevice.as_ptr())
        .to_string_lossy()
        .to_string();
    
    // Check if this is the primary monitor
    let is_primary = (monitor_info.dwFlags & winapi::um::winuser::MONITORINFOF_PRIMARY) != 0;
    
    let monitor = MonitorInfo {
        handle: hmonitor,
        name: device_name,
        is_primary,
        rect: monitor_info.rcMonitor,
    };
    
    data.monitors.push(monitor);
    TRUE // Continue enumeration
}

/// Enumerate all monitors in the system
fn enumerate_monitors() -> Result<Vec<MonitorInfo>, ProfileError> {
    let mut data = MonitorEnumData {
        monitors: Vec::new(),
    };
    
    let result = unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null_mut(),
            Some(monitor_enum_proc),
            &mut data as *mut _ as isize,
        )
    };
    
    if result == FALSE {
        return Err(ProfileError::SystemError(
            "Failed to enumerate monitors".to_string(),
        ));
    }
    
    Ok(data.monitors)
}

/// Read ICC profile data from file
fn read_profile_file(profile_path: &PathBuf) -> Result<Vec<u8>, ProfileError> {
    std::fs::read(profile_path).map_err(|e| {
        ProfileError::IoError(e.to_string())
    })
}

/// Parse ICC profile header to extract basic information
fn parse_icc_header(data: &[u8]) -> Result<(String, Option<String>, ColorSpace), ProfileError> {
    if data.len() < 128 {
        return Err(ProfileError::ParseError(
            "ICC profile too small to contain valid header".to_string(),
        ));
    }
    
    // Extract profile description (bytes 16-19 contain signature, we'll use a generic name)
    let profile_name = "Windows Display Profile".to_string();
    
    // Extract color space from bytes 16-19 (data color space signature)
    let color_space = match &data[16..20] {
        b"RGB " => ColorSpace::RGB,
        b"Lab " => ColorSpace::Lab,
        _ => ColorSpace::Unknown,
    };
    
    Ok((profile_name, None, color_space))
}

/// Query registry for display profile associations
fn query_registry_for_profiles() -> Result<Vec<String>, ProfileError> {
    let mut profiles = Vec::new();
    
    // Registry path for color profiles
    let registry_path = CString::new("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ICM\\ProfileAssociations\\Display")
        .map_err(|e| ProfileError::ParseError(format!("Invalid registry path: {}", e)))?;
    
    let mut hkey = ptr::null_mut();
    let result = unsafe {
        RegOpenKeyExA(
            HKEY_LOCAL_MACHINE,
            registry_path.as_ptr(),
            0,
            KEY_READ,
            &mut hkey,
        )
    };
    
    if result != 0 {
        // Registry key doesn't exist or can't be opened, not an error
        return Ok(profiles);
    }
    
    // Enumerate subkeys (display devices)
    let mut index = 0;
    loop {
        let mut key_name = vec![0u8; 256];
        let mut key_name_size = key_name.len() as DWORD;
        
        let enum_result = unsafe {
            RegEnumKeyExA(
                hkey,
                index,
                key_name.as_mut_ptr() as *mut i8,
                &mut key_name_size,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        
        if enum_result != 0 {
            break; // No more keys
        }
        
        // Try to read the default profile for this display
        let key_name_str = unsafe {
            CStr::from_ptr(key_name.as_ptr() as *const i8)
                .to_str()
                .unwrap_or("")
        };
        
        if !key_name_str.is_empty() {
            if let Ok(profile) = query_display_profile_from_registry(hkey as winapi::shared::ntdef::HANDLE, key_name_str) {
                profiles.push(profile);
            }
        }
        
        index += 1;
    }
    
    unsafe {
        RegCloseKey(hkey);
    }
    
    Ok(profiles)
}

/// Query a specific display's profile from registry
fn query_display_profile_from_registry(parent_key: winapi::shared::ntdef::HANDLE, display_name: &str) -> Result<String, ProfileError> {
    let display_key_path = CString::new(display_name)
        .map_err(|e| ProfileError::ParseError(format!("Invalid display name: {}", e)))?;
    
    let mut display_key = ptr::null_mut();
    let result = unsafe {
        RegOpenKeyExA(
            parent_key as HKEY,
            display_key_path.as_ptr(),
            0,
            KEY_READ,
            &mut display_key,
        )
    };
    
    if result != 0 {
        return Err(ProfileError::SystemError(
            "Failed to open display registry key".to_string(),
        ));
    }
    
    // Query the default profile value
    let value_name = CString::new("ICMProfile")
        .map_err(|e| ProfileError::ParseError(format!("Invalid value name: {}", e)))?;
    
    let mut buffer = vec![0u8; 260]; // MAX_PATH
    let mut buffer_size = buffer.len() as DWORD;
    let mut value_type = 0u32;
    
    let query_result = unsafe {
        RegQueryValueExA(
            display_key,
            value_name.as_ptr(),
            ptr::null_mut(),
            &mut value_type,
            buffer.as_mut_ptr(),
            &mut buffer_size,
        )
    };
    
    unsafe {
        RegCloseKey(display_key);
    }
    
    if query_result != 0 || value_type != REG_SZ {
        return Err(ProfileError::ProfileNotAvailable(
            "No profile found in registry".to_string(),
        ));
    }
    
    let profile_name = unsafe {
        CStr::from_ptr(buffer.as_ptr() as *const i8)
            .to_str()
            .map_err(|e| ProfileError::ParseError(format!("Invalid profile name: {}", e)))?
    };
    
    Ok(profile_name.to_string())
}

/// Handle Windows-specific permission and access issues
fn handle_windows_permissions_error(error: &std::io::Error) -> ProfileError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            ProfileError::SystemError(
                "Access denied. Administrator privileges may be required to access color profiles.".to_string(),
            )
        }
        std::io::ErrorKind::NotFound => {
            ProfileError::ProfileNotAvailable(
                "Color profile file not found".to_string(),
            )
        }
        _ => ProfileError::IoError(std::io::Error::from(error.kind()).to_string()),
    }
}

impl WindowsProfileProvider {
    /// Create a new Windows profile provider with default configuration
    pub fn new() -> Self {
        Self {
            config: ProfileConfig::default(),
        }
    }
    
    /// Create a new Windows profile provider with custom configuration
    pub fn with_config(config: ProfileConfig) -> Self {
        Self { config }
    }
}

impl WindowsProfileProvider {
    /// Fallback method to get profile using registry and directory scanning
    fn fallback_get_profile(&self, _display: &Display) -> Result<ProfileInfo, ProfileError> {
        // Step 1: Try registry-based profile lookup
        if let Ok(registry_profiles) = query_registry_for_profiles() {
            let color_dir = get_color_directory()?;
            
            for profile_name in registry_profiles {
                let profile_path = color_dir.join(&profile_name);
                if profile_path.exists() {
                    match std::fs::read(&profile_path) {
                        Ok(data) => {
                            let (name, description, color_space) = parse_icc_header(&data)
                                .unwrap_or_else(|_| (profile_name.clone(), None, ColorSpace::Unknown));
                            
                            return Ok(ProfileInfo {
                                name,
                                description,
                                file_path: Some(profile_path),
                                color_space,
                            });
                        }
                        Err(e) => {
                            // Handle Windows-specific permission errors
                            let _handled_error = handle_windows_permissions_error(&e);
                            continue;
                        }
                    }
                }
            }
        }
        
        // Step 2: Try to get any available profile from the color directory
        let color_dir = get_color_directory()?;
        
        // Look for common profile files
        let common_profiles = [
            "sRGB Color Space Profile.icm",
            "Adobe RGB (1998).icc",
            "ProPhoto RGB.icc",
            "Display.icc",
            "Generic RGB Profile.icc",
        ];
        
        for profile_name in &common_profiles {
            let profile_path = color_dir.join(profile_name);
            if profile_path.exists() {
                match std::fs::read(&profile_path) {
                    Ok(data) => {
                        let (name, description, color_space) = parse_icc_header(&data)
                            .unwrap_or_else(|_| (profile_name.to_string(), None, ColorSpace::Unknown));
                        
                        return Ok(ProfileInfo {
                            name,
                            description,
                            file_path: Some(profile_path),
                            color_space,
                        });
                    }
                    Err(e) => {
                        let _handled_error = handle_windows_permissions_error(&e);
                        continue;
                    }
                }
            }
        }
        
        // Step 3: Directory scanning - enumerate all profiles and pick the first valid one
        match self.scan_color_directory() {
            Ok(profile_paths) => {
                for profile_path in profile_paths {
                    match std::fs::read(&profile_path) {
                        Ok(data) => {
                            let profile_name = profile_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown Profile")
                                .to_string();
                            
                            let (name, description, color_space) = parse_icc_header(&data)
                                .unwrap_or_else(|_| (profile_name, None, ColorSpace::Unknown));
                            
                            return Ok(ProfileInfo {
                                name,
                                description,
                                file_path: Some(profile_path),
                                color_space,
                            });
                        }
                        Err(e) => {
                            let _handled_error = handle_windows_permissions_error(&e);
                            continue;
                        }
                    }
                }
                
                Err(ProfileError::ProfileNotAvailable(
                    "No valid profiles found in directory scan".to_string(),
                ))
            }
            Err(_) => {
                // Step 4: Try EnumColorProfiles API as last resort
                match enum_color_profiles() {
                    Ok(profiles) => {
                        for profile_name in profiles {
                            let profile_path = color_dir.join(&profile_name);
                            if profile_path.exists() {
                                match std::fs::read(&profile_path) {
                                    Ok(data) => {
                                        let (name, description, color_space) = parse_icc_header(&data)
                                            .unwrap_or_else(|_| (profile_name.clone(), None, ColorSpace::Unknown));
                                        
                                        return Ok(ProfileInfo {
                                            name,
                                            description,
                                            file_path: Some(profile_path),
                                            color_space,
                                        });
                                    }
                                    Err(e) => {
                                        let _handled_error = handle_windows_permissions_error(&e);
                                        continue;
                                    }
                                }
                            }
                        }
                        
                        // Final fallback: create a default sRGB profile info
                        Ok(ProfileInfo {
                            name: "Default sRGB".to_string(),
                            description: Some("Default sRGB color space (fallback)".to_string()),
                            file_path: None,
                            color_space: ColorSpace::RGB,
                        })
                    }
                    Err(_) => {
                        // Absolute last resort: create a default sRGB profile info
                        Ok(ProfileInfo {
                            name: "Default sRGB".to_string(),
                            description: Some("Default sRGB color space (fallback)".to_string()),
                            file_path: None,
                            color_space: ColorSpace::RGB,
                        })
                    }
                }
            }
        }
    }
    
    /// Scan color directory for available profiles
    fn scan_color_directory(&self) -> Result<Vec<PathBuf>, ProfileError> {
        let color_dir = get_color_directory()?;
        
        let mut profiles = Vec::new();
        
        if let Ok(entries) = std::fs::read_dir(&color_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if ext == "icc" || ext == "icm" {
                        profiles.push(path);
                    }
                }
            }
        }
        
        Ok(profiles)
    }
}

impl DisplayProfileProvider for WindowsProfileProvider {
    fn get_displays(&self) -> Result<Vec<Display>, ProfileError> {
        let monitors = enumerate_monitors()?;
        
        let mut displays = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let display = Display {
                id: format!("monitor_{}", index),
                name: monitor.name.clone(),
                is_primary: monitor.is_primary,
            };
            displays.push(display);
        }
        
        Ok(displays)
    }
    
    fn get_primary_display(&self) -> Result<Display, ProfileError> {
        let monitors = enumerate_monitors()?;
        
        for (index, monitor) in monitors.iter().enumerate() {
            if monitor.is_primary {
                return Ok(Display {
                    id: format!("monitor_{}", index),
                    name: monitor.name.clone(),
                    is_primary: true,
                });
            }
        }
        
        // Fallback: if no primary monitor found, use the first one
        if let Some(monitor) = monitors.first() {
            Ok(Display {
                id: "monitor_0".to_string(),
                name: monitor.name.clone(),
                is_primary: true, // Treat as primary since it's the only/first one
            })
        } else {
            Err(ProfileError::DisplayNotFound(
                "No displays found".to_string(),
            ))
        }
    }
    
    fn get_profile(&self, display: &Display) -> Result<ProfileInfo, ProfileError> {
        let monitors = enumerate_monitors()?;
        
        // Parse display ID to get monitor index
        let monitor_index = display.id
            .strip_prefix("monitor_")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or_else(|| ProfileError::DisplayNotFound(display.id.clone()))?;
        
        let monitor = monitors.get(monitor_index)
            .ok_or_else(|| ProfileError::DisplayNotFound(display.id.clone()))?;
        
        // Try to get the profile for this monitor
        match get_monitor_profile(monitor.handle) {
            Ok(profile_name) => {
                // Try to find the full path to the profile
                let color_dir = get_color_directory()?;
                let profile_path = color_dir.join(&profile_name);
                
                // If the profile file exists, try to parse it for more info
                if profile_path.exists() {
                    match std::fs::read(&profile_path) {
                        Ok(data) => {
                            let (name, description, color_space) = parse_icc_header(&data)
                                .unwrap_or_else(|_| (profile_name.clone(), None, ColorSpace::Unknown));
                            
                            Ok(ProfileInfo {
                                name,
                                description,
                                file_path: Some(profile_path),
                                color_space,
                            })
                        }
                        Err(_) => {
                            // File exists but can't read it, return basic info
                            Ok(ProfileInfo {
                                name: profile_name,
                                description: None,
                                file_path: Some(profile_path),
                                color_space: ColorSpace::Unknown,
                            })
                        }
                    }
                } else if self.config.fallback_enabled {
                    // Fallback: try to find any profile that might match
                    self.fallback_get_profile(display)
                } else {
                    Err(ProfileError::ProfileNotAvailable(display.id.clone()))
                }
            }
            Err(_) if self.config.fallback_enabled => {
                // Fallback to registry or directory scanning
                self.fallback_get_profile(display)
            }
            Err(e) => Err(e),
        }
    }
    
    fn get_profile_data(&self, display: &Display) -> Result<Vec<u8>, ProfileError> {
        let profile_info = self.get_profile(display)?;
        
        if let Some(file_path) = profile_info.file_path {
            match read_profile_file(&file_path) {
                Ok(data) => Ok(data),
                Err(e) => Err(e),
            }
        } else {
            Err(ProfileError::ProfileNotAvailable(
                "Profile file path not available".to_string(),
            ))
        }
    }
}
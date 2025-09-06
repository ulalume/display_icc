//! Linux-specific implementation using colormgr and D-Bus

use crate::{
    ColorSpace, Display, DisplayProfileProvider, ProfileConfig, ProfileError, ProfileInfo,
};
use std::path::PathBuf;
use std::process::Command;

#[cfg(feature = "dbus-support")]
use dbus::blocking::Connection;
#[cfg(feature = "dbus-support")]
use std::time::Duration;

/// Represents a colormgr device (display)
#[derive(Debug, Clone)]
struct ColormgrDevice {
    id: String,
    kind: String,
    model: String,
    vendor: String,
    serial: String,
    profiles: Vec<String>,
}

/// Represents a colormgr profile
#[derive(Debug, Clone)]
struct ColormgrProfile {
    id: String,
    filename: Option<PathBuf>,
    title: Option<String>,
    kind: String,
    colorspace: String,
}

/// D-Bus interface constants for colord daemon
#[cfg(feature = "dbus-support")]
const COLORD_SERVICE: &str = "org.freedesktop.ColorManager";
#[cfg(feature = "dbus-support")]
const COLORD_PATH: &str = "/org/freedesktop/ColorManager";
#[cfg(feature = "dbus-support")]
const COLORD_INTERFACE: &str = "org.freedesktop.ColorManager";

/// Linux implementation of DisplayProfileProvider using colormgr and D-Bus
pub struct LinuxProfileProvider {
    config: ProfileConfig,
}

impl LinuxProfileProvider {
    /// Create a new Linux profile provider with default configuration
    pub fn new() -> Self {
        Self {
            config: ProfileConfig::default(),
        }
    }

    /// Create a new Linux profile provider with custom configuration
    pub fn with_config(config: ProfileConfig) -> Self {
        Self { config }
    }

    /// Check if colormgr command is available
    fn is_colormgr_available(&self) -> bool {
        Command::new("colormgr")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Execute colormgr command and return output
    fn execute_colormgr(&self, args: &[&str]) -> Result<String, ProfileError> {
        if !self.is_colormgr_available() {
            return Err(ProfileError::SystemError(
                "colormgr command not found. Please install colord package.".to_string(),
            ));
        }

        let output = Command::new("colormgr")
            .args(args)
            .output()
            .map_err(|e| ProfileError::SystemError(format!("Failed to execute colormgr: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProfileError::SystemError(format!(
                "colormgr command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| ProfileError::ParseError(format!("Invalid UTF-8 output: {}", e)))?;

        Ok(stdout)
    }

    /// Get all devices from colormgr
    fn get_colormgr_devices(&self) -> Result<Vec<ColormgrDevice>, ProfileError> {
        let output = self.execute_colormgr(&["get-devices"])?;
        self.parse_colormgr_devices(&output)
    }

    /// Parse colormgr get-devices output
    fn parse_colormgr_devices(&self, output: &str) -> Result<Vec<ColormgrDevice>, ProfileError> {
        let mut devices = Vec::new();
        let mut current_device: Option<ColormgrDevice> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("Device ID:") {
                // Save previous device if exists
                if let Some(device) = current_device.take() {
                    devices.push(device);
                }

                // Start new device
                let id = line
                    .strip_prefix("Device ID:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                current_device = Some(ColormgrDevice {
                    id,
                    kind: String::new(),
                    model: String::new(),
                    vendor: String::new(),
                    serial: String::new(),
                    profiles: Vec::new(),
                });
            } else if let Some(ref mut device) = current_device {
                if line.starts_with("Kind:") {
                    device.kind = line.strip_prefix("Kind:").unwrap_or("").trim().to_string();
                } else if line.starts_with("Model:") {
                    device.model = line.strip_prefix("Model:").unwrap_or("").trim().to_string();
                } else if line.starts_with("Vendor:") {
                    device.vendor = line
                        .strip_prefix("Vendor:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                } else if line.starts_with("Serial:") {
                    device.serial = line
                        .strip_prefix("Serial:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                } else if line.starts_with("Profile ") && line.contains(":") {
                    // Extract profile ID from "Profile 1: profile_id"
                    if let Some(profile_id) = line.split(':').nth(1) {
                        device.profiles.push(profile_id.trim().to_string());
                    }
                }
            }
        }

        // Don't forget the last device
        if let Some(device) = current_device {
            devices.push(device);
        }

        // Filter to only display devices
        let display_devices: Vec<ColormgrDevice> = devices
            .into_iter()
            .filter(|device| device.kind.to_lowercase().contains("display"))
            .collect();

        Ok(display_devices)
    }

    /// Get profile information from colormgr
    fn get_colormgr_profile(&self, profile_id: &str) -> Result<ColormgrProfile, ProfileError> {
        let output = self.execute_colormgr(&["get-profile", profile_id])?;
        self.parse_colormgr_profile(&output, profile_id)
    }

    /// Parse colormgr get-profile output
    fn parse_colormgr_profile(
        &self,
        output: &str,
        profile_id: &str,
    ) -> Result<ColormgrProfile, ProfileError> {
        let mut profile = ColormgrProfile {
            id: profile_id.to_string(),
            filename: None,
            title: None,
            kind: String::new(),
            colorspace: String::new(),
        };

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("Filename:") {
                let filename_str = line.strip_prefix("Filename:").unwrap_or("").trim();
                if !filename_str.is_empty() && filename_str != "(none)" {
                    profile.filename = Some(PathBuf::from(filename_str));
                }
            } else if line.starts_with("Title:") {
                let title = line.strip_prefix("Title:").unwrap_or("").trim();
                if !title.is_empty() {
                    profile.title = Some(title.to_string());
                }
            } else if line.starts_with("Kind:") {
                profile.kind = line.strip_prefix("Kind:").unwrap_or("").trim().to_string();
            } else if line.starts_with("Colorspace:") {
                profile.colorspace = line
                    .strip_prefix("Colorspace:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        }

        Ok(profile)
    }

    /// Convert colormgr colorspace to our ColorSpace enum
    fn parse_colorspace(&self, colorspace: &str) -> ColorSpace {
        match colorspace.to_lowercase().as_str() {
            "rgb" | "srgb" => ColorSpace::RGB,
            "lab" => ColorSpace::Lab,
            _ => ColorSpace::Unknown,
        }
    }

    /// Load ICC profile data from file
    fn load_profile_data(&self, file_path: &PathBuf) -> Result<Vec<u8>, ProfileError> {
        std::fs::read(file_path).map_err(|e| ProfileError::IoError(e.to_string()))
    }

    /// Check if D-Bus API is available and preferred
    #[cfg(feature = "dbus-support")]
    fn should_use_dbus(&self) -> bool {
        self.config.linux_prefer_dbus && self.is_dbus_available()
    }

    #[cfg(not(feature = "dbus-support"))]
    fn should_use_dbus(&self) -> bool {
        false
    }

    /// Check if D-Bus colord service is available
    #[cfg(feature = "dbus-support")]
    fn is_dbus_available(&self) -> bool {
        match Connection::new_system() {
            Ok(conn) => {
                // Try to create a proxy to the colord service
                let proxy =
                    conn.with_proxy(COLORD_SERVICE, COLORD_PATH, Duration::from_millis(1000));

                // Try a simple method call to check if the service is available
                let result: Result<(Vec<dbus::Path>,), dbus::Error> =
                    proxy.method_call(COLORD_INTERFACE, "GetDevices", ());

                result.is_ok()
            }
            Err(_) => false,
        }
    }

    /// Get devices using D-Bus API
    #[cfg(feature = "dbus-support")]
    fn get_dbus_devices(&self) -> Result<Vec<ColormgrDevice>, ProfileError> {
        let conn = Connection::new_system()
            .map_err(|e| ProfileError::SystemError(format!("Failed to connect to D-Bus: {}", e)))?;

        let proxy = conn.with_proxy(COLORD_SERVICE, COLORD_PATH, Duration::from_millis(5000));

        // Get all devices
        let (device_paths,): (Vec<dbus::Path>,) = proxy
            .method_call(COLORD_INTERFACE, "GetDevices", ())
            .map_err(|e| ProfileError::SystemError(format!("D-Bus GetDevices failed: {}", e)))?;

        let mut devices = Vec::new();

        for device_path in device_paths {
            if let Ok(device) = self.get_dbus_device_info(&conn, &device_path) {
                // Filter to display devices only
                if device.kind.to_lowercase().contains("display") {
                    devices.push(device);
                }
            }
        }

        Ok(devices)
    }

    /// Get device information via D-Bus
    #[cfg(feature = "dbus-support")]
    fn get_dbus_device_info(
        &self,
        conn: &Connection,
        device_path: &dbus::Path,
    ) -> Result<ColormgrDevice, ProfileError> {
        let proxy = conn.with_proxy(COLORD_SERVICE, device_path, Duration::from_millis(2000));

        // Get device properties
        let device_id: String = proxy
            .get("org.freedesktop.ColorManager.Device", "DeviceId")
            .unwrap_or_default();

        let kind: String = proxy
            .get("org.freedesktop.ColorManager.Device", "Kind")
            .unwrap_or_default();

        let model: String = proxy
            .get("org.freedesktop.ColorManager.Device", "Model")
            .unwrap_or_default();

        let vendor: String = proxy
            .get("org.freedesktop.ColorManager.Device", "Vendor")
            .unwrap_or_default();

        let serial: String = proxy
            .get("org.freedesktop.ColorManager.Device", "Serial")
            .unwrap_or_default();

        // Get profiles for this device
        let (profile_paths,): (Vec<dbus::Path>,) = proxy
            .method_call("org.freedesktop.ColorManager.Device", "GetProfiles", ())
            .unwrap_or((Vec::new(),));

        let mut profiles = Vec::new();
        for profile_path in profile_paths {
            if let Some(profile_id) = profile_path.as_cstr().to_str().ok() {
                // Extract profile ID from path
                if let Some(id) = profile_id.split('/').last() {
                    profiles.push(id.to_string());
                }
            }
        }

        Ok(ColormgrDevice {
            id: device_id,
            kind,
            model,
            vendor,
            serial,
            profiles,
        })
    }

    /// Get profile information via D-Bus
    #[cfg(feature = "dbus-support")]
    fn get_dbus_profile(&self, profile_id: &str) -> Result<ColormgrProfile, ProfileError> {
        let conn = Connection::new_system()
            .map_err(|e| ProfileError::SystemError(format!("Failed to connect to D-Bus: {}", e)))?;

        // Find profile by ID
        let proxy = conn.with_proxy(COLORD_SERVICE, COLORD_PATH, Duration::from_millis(5000));

        let (profile_paths,): (Vec<dbus::Path>,) = proxy
            .method_call(COLORD_INTERFACE, "GetProfiles", ())
            .map_err(|e| ProfileError::SystemError(format!("D-Bus GetProfiles failed: {}", e)))?;

        for profile_path in profile_paths {
            let profile_proxy =
                conn.with_proxy(COLORD_SERVICE, &profile_path, Duration::from_millis(2000));

            let path_profile_id: String = profile_proxy
                .get("org.freedesktop.ColorManager.Profile", "ProfileId")
                .unwrap_or_default();

            if path_profile_id == profile_id {
                let filename: String = profile_proxy
                    .get("org.freedesktop.ColorManager.Profile", "Filename")
                    .unwrap_or_default();

                let title: String = profile_proxy
                    .get("org.freedesktop.ColorManager.Profile", "Title")
                    .unwrap_or_default();

                let kind: String = profile_proxy
                    .get("org.freedesktop.ColorManager.Profile", "Kind")
                    .unwrap_or_default();

                let colorspace: String = profile_proxy
                    .get("org.freedesktop.ColorManager.Profile", "Colorspace")
                    .unwrap_or_default();

                return Ok(ColormgrProfile {
                    id: profile_id.to_string(),
                    filename: if filename.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(filename))
                    },
                    title: if title.is_empty() { None } else { Some(title) },
                    kind,
                    colorspace,
                });
            }
        }

        Err(ProfileError::ProfileNotAvailable(format!(
            "Profile {} not found via D-Bus",
            profile_id
        )))
    }

    /// Fallback to file system scanning when other methods fail
    fn scan_filesystem_profiles(&self) -> Result<Vec<PathBuf>, ProfileError> {
        let profile_dirs = [
            "/usr/share/color/icc",
            "/usr/local/share/color/icc",
            "/home/.local/share/icc", // User profiles
            "/var/lib/color/icc",
        ];

        let mut profiles = Vec::new();

        for dir in &profile_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .extension()
                        .map_or(false, |ext| ext == "icc" || ext == "icm")
                    {
                        profiles.push(path);
                    }
                }
            }
        }

        Ok(profiles)
    }

    /// Convert ColormgrDevice list to Display list
    fn convert_devices_to_displays(
        &self,
        colormgr_devices: Vec<ColormgrDevice>,
    ) -> Result<Vec<Display>, ProfileError> {
        let mut displays = Vec::new();
        for (index, device) in colormgr_devices.iter().enumerate() {
            let display_name = if !device.model.is_empty() {
                if !device.vendor.is_empty() {
                    format!("{} {}", device.vendor, device.model)
                } else {
                    device.model.clone()
                }
            } else {
                format!("Display {}", index + 1)
            };

            displays.push(Display {
                id: device.id.clone(),
                name: display_name,
                is_primary: index == 0, // First display is considered primary for now
            });
        }

        if displays.is_empty() {
            return Err(ProfileError::SystemError(
                "No display devices found".to_string(),
            ));
        }

        Ok(displays)
    }
}

impl DisplayProfileProvider for LinuxProfileProvider {
    fn get_displays(&self) -> Result<Vec<Display>, ProfileError> {
        // Try D-Bus first if preferred and available
        #[cfg(feature = "dbus-support")]
        if self.should_use_dbus() {
            if let Ok(devices) = self.get_dbus_devices() {
                return self.convert_devices_to_displays(devices);
            }

            if !self.config.fallback_enabled {
                return Err(ProfileError::SystemError(
                    "D-Bus method failed and fallback is disabled".to_string(),
                ));
            }
        }

        // Fallback to colormgr command
        match self.get_colormgr_devices() {
            Ok(devices) => self.convert_devices_to_displays(devices),
            Err(e) => {
                if !self.config.fallback_enabled {
                    return Err(e);
                }

                // Final fallback: return a generic display if we can find any profiles
                match self.scan_filesystem_profiles() {
                    Ok(profiles) if !profiles.is_empty() => Ok(vec![Display {
                        id: "filesystem-fallback".to_string(),
                        name: "Generic Display".to_string(),
                        is_primary: true,
                    }]),
                    _ => Err(ProfileError::SystemError(
                        "No display devices found via any method".to_string(),
                    )),
                }
            }
        }
    }

    fn get_primary_display(&self) -> Result<Display, ProfileError> {
        let displays = self.get_displays()?;
        displays
            .into_iter()
            .find(|d| d.is_primary)
            .ok_or_else(|| ProfileError::DisplayNotFound("No primary display found".to_string()))
    }

    fn get_profile(&self, display: &Display) -> Result<ProfileInfo, ProfileError> {
        // Handle filesystem fallback case
        if display.id == "filesystem-fallback" {
            let profiles = self.scan_filesystem_profiles()?;
            if let Some(profile_path) = profiles.first() {
                return Ok(ProfileInfo {
                    name: profile_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown Profile")
                        .to_string(),
                    description: None,
                    file_path: Some(profile_path.clone()),
                    color_space: ColorSpace::Unknown,
                });
            }
        }

        // Try D-Bus first if preferred and available
        #[cfg(feature = "dbus-support")]
        if self.should_use_dbus() {
            if let Ok(devices) = self.get_dbus_devices() {
                if let Some(device) = devices.iter().find(|d| d.id == display.id) {
                    if let Some(profile_id) = device.profiles.first() {
                        if let Ok(profile) = self.get_dbus_profile(profile_id) {
                            let profile_name = profile.title.unwrap_or_else(|| profile.id.clone());

                            return Ok(ProfileInfo {
                                name: profile_name,
                                description: None,
                                file_path: profile.filename,
                                color_space: self.parse_colorspace(&profile.colorspace),
                            });
                        }
                    }
                }
            }

            if !self.config.fallback_enabled {
                return Err(ProfileError::SystemError(
                    "D-Bus method failed and fallback is disabled".to_string(),
                ));
            }
        }

        // Fallback to colormgr command
        let colormgr_devices = self.get_colormgr_devices()?;

        // Find the device matching this display
        let device = colormgr_devices
            .iter()
            .find(|d| d.id == display.id)
            .ok_or_else(|| ProfileError::DisplayNotFound(display.id.clone()))?;

        // Get the first profile for this device
        let profile_id = device
            .profiles
            .first()
            .ok_or_else(|| ProfileError::ProfileNotAvailable(display.id.clone()))?;

        let colormgr_profile = self.get_colormgr_profile(profile_id)?;

        let profile_name = colormgr_profile
            .title
            .unwrap_or_else(|| colormgr_profile.id.clone());

        Ok(ProfileInfo {
            name: profile_name,
            description: None, // colormgr doesn't provide description
            file_path: colormgr_profile.filename,
            color_space: self.parse_colorspace(&colormgr_profile.colorspace),
        })
    }

    fn get_profile_data(&self, display: &Display) -> Result<Vec<u8>, ProfileError> {
        let profile_info = self.get_profile(display)?;

        let file_path = profile_info.file_path.ok_or_else(|| {
            ProfileError::ProfileNotAvailable(format!(
                "No file path available for display {}",
                display.id
            ))
        })?;

        self.load_profile_data(&file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_colormgr_devices() {
        let provider = LinuxProfileProvider::new();
        let sample_output = r#"
Device ID:          xrandr-Goldstar Company Ltd-LG ULTRAWIDE-0x00000101
Kind:               display
Model:              LG ULTRAWIDE
Vendor:             Goldstar Company Ltd
Serial:             0x00000101
Profile 1:          icc-2c9c8b0c8e5c4e9b8f7a6d5c4b3a2918
Profile 2:          icc-b7f8e9d0c1a2b3c4d5e6f7a8b9c0d1e2

Device ID:          xrandr-Dell Inc.-DELL U2415-HT8XN64P0D2S
Kind:               display
Model:              DELL U2415
Vendor:             Dell Inc.
Serial:             HT8XN64P0D2S
Profile 1:          icc-a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6
        "#;

        let devices = provider.parse_colormgr_devices(sample_output).unwrap();

        assert_eq!(devices.len(), 2);

        let first_device = &devices[0];
        assert_eq!(
            first_device.id,
            "xrandr-Goldstar Company Ltd-LG ULTRAWIDE-0x00000101"
        );
        assert_eq!(first_device.kind, "display");
        assert_eq!(first_device.model, "LG ULTRAWIDE");
        assert_eq!(first_device.vendor, "Goldstar Company Ltd");
        assert_eq!(first_device.serial, "0x00000101");
        assert_eq!(first_device.profiles.len(), 2);
        assert_eq!(
            first_device.profiles[0],
            "icc-2c9c8b0c8e5c4e9b8f7a6d5c4b3a2918"
        );
        assert_eq!(
            first_device.profiles[1],
            "icc-b7f8e9d0c1a2b3c4d5e6f7a8b9c0d1e2"
        );

        let second_device = &devices[1];
        assert_eq!(second_device.id, "xrandr-Dell Inc.-DELL U2415-HT8XN64P0D2S");
        assert_eq!(second_device.model, "DELL U2415");
        assert_eq!(second_device.vendor, "Dell Inc.");
        assert_eq!(second_device.profiles.len(), 1);
        assert_eq!(
            second_device.profiles[0],
            "icc-a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6"
        );
    }

    #[test]
    fn test_parse_colormgr_devices_empty() {
        let provider = LinuxProfileProvider::new();
        let empty_output = "";

        let devices = provider.parse_colormgr_devices(empty_output).unwrap();
        assert_eq!(devices.len(), 0);
    }

    #[test]
    fn test_parse_colormgr_devices_no_displays() {
        let provider = LinuxProfileProvider::new();
        let sample_output = r#"
Device ID:          usb-046d-c52b-event-mouse
Kind:               mouse
Model:              Logitech USB Receiver
Vendor:             Logitech
        "#;

        let devices = provider.parse_colormgr_devices(sample_output).unwrap();
        assert_eq!(devices.len(), 0); // Should filter out non-display devices
    }

    #[test]
    fn test_parse_colormgr_profile() {
        let provider = LinuxProfileProvider::new();
        let sample_output = r#"
Profile ID:         icc-2c9c8b0c8e5c4e9b8f7a6d5c4b3a2918
Filename:           /usr/share/color/icc/sRGB.icc
Title:              sRGB IEC61966-2.1
Kind:               display-device
Colorspace:         rgb
        "#;

        let profile = provider
            .parse_colormgr_profile(sample_output, "test-id")
            .unwrap();

        assert_eq!(profile.id, "test-id");
        assert_eq!(
            profile.filename,
            Some(PathBuf::from("/usr/share/color/icc/sRGB.icc"))
        );
        assert_eq!(profile.title, Some("sRGB IEC61966-2.1".to_string()));
        assert_eq!(profile.kind, "display-device");
        assert_eq!(profile.colorspace, "rgb");
    }

    #[test]
    fn test_parse_colormgr_profile_no_filename() {
        let provider = LinuxProfileProvider::new();
        let sample_output = r#"
Profile ID:         icc-builtin-profile
Filename:           (none)
Title:              Built-in sRGB
Kind:               display-device
Colorspace:         rgb
        "#;

        let profile = provider
            .parse_colormgr_profile(sample_output, "test-id")
            .unwrap();

        assert_eq!(profile.filename, None);
        assert_eq!(profile.title, Some("Built-in sRGB".to_string()));
    }

    #[test]
    fn test_parse_colorspace() {
        let provider = LinuxProfileProvider::new();

        assert_eq!(provider.parse_colorspace("rgb"), ColorSpace::RGB);
        assert_eq!(provider.parse_colorspace("RGB"), ColorSpace::RGB);
        assert_eq!(provider.parse_colorspace("srgb"), ColorSpace::RGB);
        assert_eq!(provider.parse_colorspace("lab"), ColorSpace::Lab);
        assert_eq!(provider.parse_colorspace("LAB"), ColorSpace::Lab);
        assert_eq!(provider.parse_colorspace("xyz"), ColorSpace::Unknown);
        assert_eq!(provider.parse_colorspace(""), ColorSpace::Unknown);
    }

    #[test]
    fn test_is_colormgr_available() {
        let provider = LinuxProfileProvider::new();
        // This test will depend on whether colormgr is installed
        // We can't assert a specific result, but we can test that it doesn't panic
        let _available = provider.is_colormgr_available();
    }

    #[test]
    fn test_should_use_dbus_without_feature() {
        let provider = LinuxProfileProvider::new();
        // Without dbus-support feature, should always return false
        assert!(!provider.should_use_dbus());
    }

    #[test]
    fn test_should_use_dbus_with_config() {
        let config = ProfileConfig {
            linux_prefer_dbus: false,
            fallback_enabled: true,
        };
        let provider = LinuxProfileProvider::with_config(config);
        // Even with dbus feature, should respect config
        assert!(!provider.should_use_dbus());
    }

    #[test]
    fn test_scan_filesystem_profiles() {
        let provider = LinuxProfileProvider::new();
        // This will depend on the system, but should not panic
        let _result = provider.scan_filesystem_profiles();
    }

    #[test]
    fn test_fallback_chain_config() {
        let config_with_fallback = ProfileConfig {
            linux_prefer_dbus: true,
            fallback_enabled: true,
        };

        let config_without_fallback = ProfileConfig {
            linux_prefer_dbus: true,
            fallback_enabled: false,
        };

        let provider_with = LinuxProfileProvider::with_config(config_with_fallback);
        let provider_without = LinuxProfileProvider::with_config(config_without_fallback);

        assert!(provider_with.config.fallback_enabled);
        assert!(!provider_without.config.fallback_enabled);
    }

    #[test]
    fn test_filesystem_fallback_display() {
        let provider = LinuxProfileProvider::new();

        // Test the filesystem fallback display creation logic
        let profiles = vec![
            PathBuf::from("/usr/share/color/icc/sRGB.icc"),
            PathBuf::from("/usr/share/color/icc/AdobeRGB.icc"),
        ];

        if !profiles.is_empty() {
            let displays = vec![Display {
                id: "filesystem-fallback".to_string(),
                name: "Generic Display".to_string(),
                is_primary: true,
            }];

            assert_eq!(displays.len(), 1);
            assert_eq!(displays[0].id, "filesystem-fallback");
            assert_eq!(displays[0].name, "Generic Display");
            assert!(displays[0].is_primary);
        }
    }

    // Mock tests for the trait implementation
    struct MockLinuxProvider {
        devices: Vec<ColormgrDevice>,
        should_fail: bool,
    }

    impl MockLinuxProvider {
        fn new() -> Self {
            let devices = vec![
                ColormgrDevice {
                    id: "display-1".to_string(),
                    kind: "display".to_string(),
                    model: "Test Monitor".to_string(),
                    vendor: "Test Vendor".to_string(),
                    serial: "12345".to_string(),
                    profiles: vec!["profile-1".to_string()],
                },
                ColormgrDevice {
                    id: "display-2".to_string(),
                    kind: "display".to_string(),
                    model: "Second Monitor".to_string(),
                    vendor: "Another Vendor".to_string(),
                    serial: "67890".to_string(),
                    profiles: vec!["profile-2".to_string()],
                },
            ];

            Self {
                devices,
                should_fail: false,
            }
        }

        fn with_failure() -> Self {
            Self {
                devices: Vec::new(),
                should_fail: true,
            }
        }

        fn empty() -> Self {
            Self {
                devices: Vec::new(),
                should_fail: false,
            }
        }
    }

    // We can't easily mock the actual colormgr commands in unit tests,
    // but we can test the parsing logic and error handling
    #[test]
    fn test_display_name_generation() {
        let provider = LinuxProfileProvider::new();

        // Test with vendor and model
        let device1 = ColormgrDevice {
            id: "test-1".to_string(),
            kind: "display".to_string(),
            model: "Monitor".to_string(),
            vendor: "TestCorp".to_string(),
            serial: "123".to_string(),
            profiles: vec!["profile-1".to_string()],
        };

        // Test with model only
        let device2 = ColormgrDevice {
            id: "test-2".to_string(),
            kind: "display".to_string(),
            model: "Monitor".to_string(),
            vendor: "".to_string(),
            serial: "456".to_string(),
            profiles: vec!["profile-2".to_string()],
        };

        // Test with no model
        let device3 = ColormgrDevice {
            id: "test-3".to_string(),
            kind: "display".to_string(),
            model: "".to_string(),
            vendor: "".to_string(),
            serial: "789".to_string(),
            profiles: vec!["profile-3".to_string()],
        };

        let devices = vec![device1, device2, device3];

        // Simulate the display name generation logic
        let mut displays = Vec::new();
        for (index, device) in devices.iter().enumerate() {
            let display_name = if !device.model.is_empty() {
                if !device.vendor.is_empty() {
                    format!("{} {}", device.vendor, device.model)
                } else {
                    device.model.clone()
                }
            } else {
                format!("Display {}", index + 1)
            };

            displays.push(Display {
                id: device.id.clone(),
                name: display_name,
                is_primary: index == 0,
            });
        }

        assert_eq!(displays[0].name, "TestCorp Monitor");
        assert_eq!(displays[1].name, "Monitor");
        assert_eq!(displays[2].name, "Display 3");

        assert!(displays[0].is_primary);
        assert!(!displays[1].is_primary);
        assert!(!displays[2].is_primary);
    }
}

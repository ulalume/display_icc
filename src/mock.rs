//! Mock implementations for testing

use crate::{Display, DisplayProfileProvider, ProfileError, ProfileInfo, ColorSpace};
use std::collections::HashMap;
use std::path::PathBuf;

/// Mock implementation of DisplayProfileProvider for testing
#[derive(Debug, Clone)]
pub struct MockProfileProvider {
    displays: Vec<Display>,
    profiles: HashMap<String, ProfileInfo>,
    profile_data: HashMap<String, Vec<u8>>,
    should_fail: HashMap<String, ProfileError>,
}

impl MockProfileProvider {
    /// Create a new mock provider with no displays
    pub fn new() -> Self {
        Self {
            displays: Vec::new(),
            profiles: HashMap::new(),
            profile_data: HashMap::new(),
            should_fail: HashMap::new(),
        }
    }

    /// Create a mock provider with typical test data
    pub fn with_test_data() -> Self {
        let mut provider = Self::new();
        
        // Add primary display
        let primary_display = Display {
            id: "primary".to_string(),
            name: "Primary Display".to_string(),
            is_primary: true,
        };
        
        let primary_profile = ProfileInfo {
            name: "sRGB IEC61966-2.1".to_string(),
            description: Some("Standard RGB color space".to_string()),
            file_path: Some(PathBuf::from("/System/Library/ColorSync/Profiles/sRGB Profile.icc")),
            color_space: ColorSpace::RGB,
        };
        
        // Create minimal valid ICC profile data
        let mut icc_data = vec![0u8; 128];
        icc_data[0..4].copy_from_slice(&1024u32.to_be_bytes()); // profile size
        icc_data[12..16].copy_from_slice(b"mntr"); // device class
        icc_data[16..20].copy_from_slice(b"RGB "); // color space
        icc_data[20..24].copy_from_slice(b"XYZ "); // connection space
        
        provider.add_display(primary_display);
        provider.set_profile("primary", primary_profile);
        provider.set_profile_data("primary", icc_data);
        
        // Add secondary display
        let secondary_display = Display {
            id: "secondary".to_string(),
            name: "Secondary Display".to_string(),
            is_primary: false,
        };
        
        let secondary_profile = ProfileInfo {
            name: "Display P3".to_string(),
            description: Some("Display P3 color space".to_string()),
            file_path: Some(PathBuf::from("/System/Library/ColorSync/Profiles/Display P3.icc")),
            color_space: ColorSpace::RGB,
        };
        
        let mut p3_icc_data = vec![0u8; 128];
        p3_icc_data[0..4].copy_from_slice(&2048u32.to_be_bytes());
        p3_icc_data[12..16].copy_from_slice(b"mntr");
        p3_icc_data[16..20].copy_from_slice(b"RGB ");
        p3_icc_data[20..24].copy_from_slice(b"XYZ ");
        
        provider.add_display(secondary_display);
        provider.set_profile("secondary", secondary_profile);
        provider.set_profile_data("secondary", p3_icc_data);
        
        provider
    }

    /// Add a display to the mock provider
    pub fn add_display(&mut self, display: Display) {
        self.displays.push(display);
    }

    /// Set profile information for a display
    pub fn set_profile(&mut self, display_id: &str, profile: ProfileInfo) {
        self.profiles.insert(display_id.to_string(), profile);
    }

    /// Set profile data for a display
    pub fn set_profile_data(&mut self, display_id: &str, data: Vec<u8>) {
        self.profile_data.insert(display_id.to_string(), data);
    }

    /// Configure a method to fail for a specific display
    pub fn set_failure(&mut self, display_id: &str, error: ProfileError) {
        self.should_fail.insert(display_id.to_string(), error);
    }

    /// Remove all displays
    pub fn clear_displays(&mut self) {
        self.displays.clear();
        self.profiles.clear();
        self.profile_data.clear();
        self.should_fail.clear();
    }
}

impl Default for MockProfileProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayProfileProvider for MockProfileProvider {
    fn get_displays(&self) -> Result<Vec<Display>, ProfileError> {
        if self.should_fail.contains_key("get_displays") {
            return Err(self.should_fail["get_displays"].clone());
        }
        Ok(self.displays.clone())
    }

    fn get_primary_display(&self) -> Result<Display, ProfileError> {
        if self.should_fail.contains_key("get_primary_display") {
            return Err(self.should_fail["get_primary_display"].clone());
        }
        
        self.displays
            .iter()
            .find(|d| d.is_primary)
            .cloned()
            .ok_or_else(|| ProfileError::DisplayNotFound("No primary display found".to_string()))
    }

    fn get_profile(&self, display: &Display) -> Result<ProfileInfo, ProfileError> {
        if let Some(error) = self.should_fail.get(&display.id) {
            return Err(error.clone());
        }
        
        self.profiles
            .get(&display.id)
            .cloned()
            .ok_or_else(|| ProfileError::ProfileNotAvailable(display.id.clone()))
    }

    fn get_profile_data(&self, display: &Display) -> Result<Vec<u8>, ProfileError> {
        if let Some(error) = self.should_fail.get(&display.id) {
            return Err(error.clone());
        }
        
        self.profile_data
            .get(&display.id)
            .cloned()
            .ok_or_else(|| ProfileError::ProfileNotAvailable(display.id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider_empty() {
        let provider = MockProfileProvider::new();
        let displays = provider.get_displays().unwrap();
        assert!(displays.is_empty());
        
        let primary_result = provider.get_primary_display();
        assert!(primary_result.is_err());
    }

    #[test]
    fn test_mock_provider_with_test_data() {
        let provider = MockProfileProvider::with_test_data();
        
        // Test get_displays
        let displays = provider.get_displays().unwrap();
        assert_eq!(displays.len(), 2);
        
        // Test get_primary_display
        let primary = provider.get_primary_display().unwrap();
        assert_eq!(primary.id, "primary");
        assert!(primary.is_primary);
        
        // Test get_profile
        let profile = provider.get_profile(&primary).unwrap();
        assert_eq!(profile.name, "sRGB IEC61966-2.1");
        assert_eq!(profile.color_space, ColorSpace::RGB);
        
        // Test get_profile_data
        let data = provider.get_profile_data(&primary).unwrap();
        assert_eq!(data.len(), 128);
        
        // Verify ICC header
        let profile_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(profile_size, 1024);
    }

    #[test]
    fn test_mock_provider_add_display() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "test".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };
        
        provider.add_display(display.clone());
        
        let displays = provider.get_displays().unwrap();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0], display);
    }

    #[test]
    fn test_mock_provider_set_profile() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "test".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };
        
        let profile = ProfileInfo {
            name: "Test Profile".to_string(),
            description: None,
            file_path: None,
            color_space: ColorSpace::RGB,
        };
        
        provider.add_display(display.clone());
        provider.set_profile("test", profile.clone());
        
        let retrieved_profile = provider.get_profile(&display).unwrap();
        assert_eq!(retrieved_profile, profile);
    }

    #[test]
    fn test_mock_provider_set_profile_data() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "test".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };
        
        let test_data = vec![1, 2, 3, 4, 5];
        
        provider.add_display(display.clone());
        provider.set_profile_data("test", test_data.clone());
        
        let retrieved_data = provider.get_profile_data(&display).unwrap();
        assert_eq!(retrieved_data, test_data);
    }

    #[test]
    fn test_mock_provider_failures() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "test".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };
        
        provider.add_display(display.clone());
        provider.set_failure("test", ProfileError::SystemError("Mock error".to_string()));
        
        let profile_result = provider.get_profile(&display);
        assert!(profile_result.is_err());
        
        if let Err(ProfileError::SystemError(msg)) = profile_result {
            assert_eq!(msg, "Mock error");
        } else {
            panic!("Expected SystemError");
        }
    }

    #[test]
    fn test_mock_provider_profile_not_available() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "test".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };
        
        provider.add_display(display.clone());
        // Don't set profile - should return ProfileNotAvailable
        
        let profile_result = provider.get_profile(&display);
        assert!(profile_result.is_err());
        
        if let Err(ProfileError::ProfileNotAvailable(id)) = profile_result {
            assert_eq!(id, "test");
        } else {
            panic!("Expected ProfileNotAvailable");
        }
    }

    #[test]
    fn test_mock_provider_clear_displays() {
        let mut provider = MockProfileProvider::with_test_data();
        
        // Verify we have test data
        assert_eq!(provider.get_displays().unwrap().len(), 2);
        
        // Clear and verify empty
        provider.clear_displays();
        assert!(provider.get_displays().unwrap().is_empty());
        assert!(provider.get_primary_display().is_err());
    }
}
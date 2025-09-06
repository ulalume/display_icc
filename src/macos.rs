//! macOS-specific implementation using CoreGraphics framework

use crate::{Display, DisplayProfileProvider, ProfileConfig, ProfileError, ProfileInfo, ColorSpace};
use core_graphics::display::CGMainDisplayID;
use core_foundation::base::{TCFType, CFRelease, CFTypeRef};
use core_foundation::data::{CFData, CFDataRef};
use core_foundation::string::{CFString, CFStringRef};

// Raw CoreGraphics types
type CGColorSpaceRef = *mut std::ffi::c_void;

// External CoreGraphics functions not available in core-graphics crate
extern "C" {
    /// Get active display list
    fn CGGetActiveDisplayList(max_displays: u32, active_displays: *mut u32, display_count: *mut u32) -> i32;
    
    /// Copy the color space associated with a display
    fn CGDisplayCopyColorSpace(display: u32) -> CGColorSpaceRef;
    
    /// Copy ICC profile data from a color space
    fn CGColorSpaceCopyICCData(space: CGColorSpaceRef) -> CFDataRef;
    
    /// Get the name of a color space
    fn CGColorSpaceCopyName(space: CGColorSpaceRef) -> CFStringRef;
    
    /// Check if a display is the main display
    fn CGDisplayIsMain(display: u32) -> bool;
}



/// Safe wrapper around CoreGraphics display enumeration
fn get_active_displays() -> Result<Vec<u32>, ProfileError> {
    const MAX_DISPLAYS: u32 = 32;
    let mut displays = vec![0u32; MAX_DISPLAYS as usize];
    let mut display_count = 0u32;
    
    unsafe {
        let result = CGGetActiveDisplayList(
            MAX_DISPLAYS,
            displays.as_mut_ptr(),
            &mut display_count
        );
        
        if result != 0 {
            return Err(ProfileError::SystemError(
                format!("CGGetActiveDisplayList failed with code: {}", result)
            ));
        }
    }
    
    if display_count == 0 {
        return Err(ProfileError::SystemError("No active displays found".to_string()));
    }
    
    displays.truncate(display_count as usize);
    Ok(displays)
}

/// Safe wrapper around CGDisplayCopyColorSpace
fn copy_display_color_space(display_id: u32) -> Result<CGColorSpaceRef, ProfileError> {
    unsafe {
        let color_space_ref = CGDisplayCopyColorSpace(display_id);
        if color_space_ref.is_null() {
            return Err(ProfileError::ProfileNotAvailable(format!("Display {}", display_id)));
        }
        
        Ok(color_space_ref)
    }
}

/// Safe wrapper around CGColorSpaceCopyICCData
fn copy_icc_data_from_color_space(color_space_ref: CGColorSpaceRef) -> Result<Vec<u8>, ProfileError> {
    unsafe {
        let data_ref = CGColorSpaceCopyICCData(color_space_ref);
        if data_ref.is_null() {
            return Err(ProfileError::ProfileNotAvailable("No ICC data available".to_string()));
        }
        
        let cf_data = CFData::wrap_under_create_rule(data_ref);
        let bytes = cf_data.bytes();
        Ok(bytes.to_vec())
    }
}

/// Safe wrapper around CGColorSpaceCopyName
fn copy_color_space_name(color_space_ref: CGColorSpaceRef) -> Result<String, ProfileError> {
    unsafe {
        let name_ref = CGColorSpaceCopyName(color_space_ref);
        if name_ref.is_null() {
            // Fallback to a generic name based on color space type
            return Ok("Display Profile".to_string());
        }
        
        let cf_string = CFString::wrap_under_create_rule(name_ref);
        let name = cf_string.to_string();
        
        // If the name is empty or just whitespace, provide a fallback
        if name.trim().is_empty() {
            Ok("Display Profile".to_string())
        } else {
            Ok(name)
        }
    }
}

/// Get display name from display ID
fn get_display_name(display_id: u32) -> String {
    unsafe {
        let is_main = CGDisplayIsMain(display_id);
        if is_main {
            "Built-in Display".to_string()
        } else {
            // For external displays, we could potentially get more info
            // but for now, use a descriptive name
            format!("External Display {}", display_id)
        }
    }
}

/// Determine color space from ICC profile data
fn determine_color_space(icc_data: &[u8]) -> ColorSpace {
    if icc_data.len() < 20 {
        return ColorSpace::Unknown;
    }
    
    // Check ICC profile header for color space signature (bytes 16-19)
    match &icc_data[16..20] {
        b"RGB " => ColorSpace::RGB,
        b"Lab " => ColorSpace::Lab,
        _ => ColorSpace::Unknown,
    }
}

/// Known Apple display profiles for fallback
#[derive(Debug, Clone)]
struct AppleDisplayProfile {
    name: String,
    description: String,
    color_space: ColorSpace,
    // Minimal ICC header for fallback (simplified)
    icc_data: Vec<u8>,
}

impl AppleDisplayProfile {
    fn srgb() -> Self {
        // Create a minimal sRGB ICC profile header
        let mut icc_data = vec![0u8; 128]; // Minimal ICC header size
        
        // Profile size (128 bytes)
        icc_data[0..4].copy_from_slice(&128u32.to_be_bytes());
        // Preferred CMM type
        icc_data[4..8].copy_from_slice(b"ADBE");
        // Profile version
        icc_data[8..12].copy_from_slice(&0x02100000u32.to_be_bytes());
        // Device class (display)
        icc_data[12..16].copy_from_slice(b"mntr");
        // Color space (RGB)
        icc_data[16..20].copy_from_slice(b"RGB ");
        // Profile connection space (XYZ)
        icc_data[20..24].copy_from_slice(b"XYZ ");
        
        Self {
            name: "sRGB IEC61966-2.1".to_string(),
            description: "Standard RGB color space".to_string(),
            color_space: ColorSpace::RGB,
            icc_data,
        }
    }
    
    #[allow(dead_code)]
    fn display_p3() -> Self {
        // Create a minimal Display P3 ICC profile header
        let mut icc_data = vec![0u8; 128];
        
        // Profile size (128 bytes)
        icc_data[0..4].copy_from_slice(&128u32.to_be_bytes());
        // Preferred CMM type
        icc_data[4..8].copy_from_slice(b"APPL");
        // Profile version
        icc_data[8..12].copy_from_slice(&0x02100000u32.to_be_bytes());
        // Device class (display)
        icc_data[12..16].copy_from_slice(b"mntr");
        // Color space (RGB)
        icc_data[16..20].copy_from_slice(b"RGB ");
        // Profile connection space (XYZ)
        icc_data[20..24].copy_from_slice(b"XYZ ");
        
        Self {
            name: "Display P3".to_string(),
            description: "Display P3 color space".to_string(),
            color_space: ColorSpace::RGB,
            icc_data,
        }
    }
    
    fn color_lcd() -> Self {
        // Create a minimal Color LCD ICC profile header
        let mut icc_data = vec![0u8; 128];
        
        // Profile size (128 bytes)
        icc_data[0..4].copy_from_slice(&128u32.to_be_bytes());
        // Preferred CMM type
        icc_data[4..8].copy_from_slice(b"APPL");
        // Profile version
        icc_data[8..12].copy_from_slice(&0x02100000u32.to_be_bytes());
        // Device class (display)
        icc_data[12..16].copy_from_slice(b"mntr");
        // Color space (RGB)
        icc_data[16..20].copy_from_slice(b"RGB ");
        // Profile connection space (XYZ)
        icc_data[20..24].copy_from_slice(b"XYZ ");
        
        Self {
            name: "Color LCD".to_string(),
            description: "Apple Color LCD profile".to_string(),
            color_space: ColorSpace::RGB,
            icc_data,
        }
    }
}

/// Get fallback profile for a display
fn get_fallback_profile(display: &Display) -> AppleDisplayProfile {
    // For built-in displays, prefer Color LCD or sRGB
    if display.is_primary && display.name.contains("Built-in") {
        AppleDisplayProfile::color_lcd()
    } else {
        // For external displays, use sRGB as a safe default
        AppleDisplayProfile::srgb()
    }
}

/// Try to get profile with fallback mechanisms
fn get_profile_with_fallback(display: &Display, config: &ProfileConfig) -> Result<ProfileInfo, ProfileError> {
    // Try to parse display ID
    let display_id = match display.id.parse::<u32>() {
        Ok(id) => id,
        Err(_) if config.fallback_enabled => {
            // If display ID parsing fails and fallback is enabled, use fallback
            let fallback = get_fallback_profile(display);
            return Ok(ProfileInfo {
                name: fallback.name,
                description: Some(fallback.description),
                file_path: None,
                color_space: fallback.color_space,
            });
        }
        Err(_) => return Err(ProfileError::DisplayNotFound(display.id.clone())),
    };
    
    // First, try the normal CoreGraphics approach
    match copy_display_color_space(display_id) {
        Ok(color_space_ref) => {
            let profile_name = copy_color_space_name(color_space_ref)
                .unwrap_or_else(|_| "Display Profile".to_string());
            
            let color_space_type = match copy_icc_data_from_color_space(color_space_ref) {
                Ok(icc_data) => determine_color_space(&icc_data),
                Err(_) => ColorSpace::RGB, // Default to RGB if we can't determine
            };
            
            unsafe {
                CFRelease(color_space_ref as CFTypeRef);
            }
            
            return Ok(ProfileInfo {
                name: profile_name,
                description: Some(format!("Color profile for {}", display.name)),
                file_path: None,
                color_space: color_space_type,
            });
        }
        Err(_) if config.fallback_enabled => {
            // Fallback to known Apple profiles
            let fallback = get_fallback_profile(display);
            return Ok(ProfileInfo {
                name: fallback.name,
                description: Some(fallback.description),
                file_path: None,
                color_space: fallback.color_space,
            });
        }
        Err(e) => return Err(e),
    }
}

/// Try to get profile data with fallback mechanisms
fn get_profile_data_with_fallback(display: &Display, config: &ProfileConfig) -> Result<Vec<u8>, ProfileError> {
    // Try to parse display ID
    let display_id = match display.id.parse::<u32>() {
        Ok(id) => id,
        Err(_) if config.fallback_enabled => {
            // If display ID parsing fails and fallback is enabled, use fallback
            let fallback = get_fallback_profile(display);
            return Ok(fallback.icc_data);
        }
        Err(_) => return Err(ProfileError::DisplayNotFound(display.id.clone())),
    };
    
    // First, try the normal CoreGraphics approach
    match copy_display_color_space(display_id) {
        Ok(color_space_ref) => {
            let result = copy_icc_data_from_color_space(color_space_ref);
            
            unsafe {
                CFRelease(color_space_ref as CFTypeRef);
            }
            
            match result {
                Ok(data) => return Ok(data),
                Err(_) if config.fallback_enabled => {
                    // Fall through to fallback mechanism
                }
                Err(e) => return Err(e),
            }
        }
        Err(_) if !config.fallback_enabled => {
            return Err(ProfileError::ProfileNotAvailable(display.id.clone()));
        }
        Err(_) => {
            // Fall through to fallback mechanism
        }
    }
    
    // Fallback to known Apple profiles
    if config.fallback_enabled {
        let fallback = get_fallback_profile(display);
        Ok(fallback.icc_data)
    } else {
        Err(ProfileError::ProfileNotAvailable(display.id.clone()))
    }
}

/// macOS implementation of DisplayProfileProvider using CoreGraphics
pub struct MacOSProfileProvider {
    config: ProfileConfig,
}

impl MacOSProfileProvider {
    /// Create a new macOS profile provider with default configuration
    pub fn new() -> Self {
        Self {
            config: ProfileConfig::default(),
        }
    }
    
    /// Create a new macOS profile provider with custom configuration
    pub fn with_config(config: ProfileConfig) -> Self {
        Self { config }
    }
}

impl DisplayProfileProvider for MacOSProfileProvider {
    fn get_displays(&self) -> Result<Vec<Display>, ProfileError> {
        let display_ids = get_active_displays()?;
        let mut displays = Vec::new();
        
        for display_id in display_ids {
            let display = Display {
                id: display_id.to_string(),
                name: get_display_name(display_id),
                is_primary: unsafe { CGDisplayIsMain(display_id) },
            };
            displays.push(display);
        }
        
        Ok(displays)
    }
    
    fn get_primary_display(&self) -> Result<Display, ProfileError> {
        let main_display_id = unsafe { CGMainDisplayID() };
        
        Ok(Display {
            id: main_display_id.to_string(),
            name: get_display_name(main_display_id),
            is_primary: true,
        })
    }
    
    fn get_profile(&self, display: &Display) -> Result<ProfileInfo, ProfileError> {
        get_profile_with_fallback(display, &self.config)
    }
    
    fn get_profile_data(&self, display: &Display) -> Result<Vec<u8>, ProfileError> {
        get_profile_data_with_fallback(display, &self.config)
    }
}
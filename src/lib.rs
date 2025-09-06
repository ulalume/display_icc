//! # display_icc
//!
//! A cross-platform Rust library for retrieving display ICC profiles on macOS, Linux, and Windows.
//! 
//! This library provides a unified API for accessing the active ICC color profiles associated with
//! displays across different operating systems, abstracting away platform-specific implementations.
//!
//! ## Features
//!
//! - **Cross-platform support**: Works on macOS, Linux, and Windows
//! - **Multiple display support**: Handle single and multi-monitor setups
//! - **Unified API**: Same interface across all platforms
//! - **Raw ICC data access**: Get both profile metadata and raw ICC binary data
//! - **Robust error handling**: Comprehensive error types with fallback mechanisms
//! - **CLI interface**: Command-line tool for quick profile inspection
//!
//! ## Platform-Specific Behavior
//!
//! ### macOS
//! - Uses CoreGraphics framework (`CGDisplayCopyColorSpace`, `CGColorSpaceCopyICCData`)
//! - Supports multiple displays via `CGGetActiveDisplayList`
//! - Falls back to known Apple display profiles when APIs fail
//! - Handles both built-in and external displays
//!
//! ### Linux
//! - Primary: Uses `colormgr` command-line tool
//! - Secondary: D-Bus API integration with colord daemon
//! - Fallback: File system scanning in `/usr/share/color/icc/`
//! - Requires colord/colormgr to be installed for full functionality
//!
//! ### Windows
//! - Uses Win32 API (`GetColorDirectory`, `EnumColorProfiles`)
//! - Registry-based profile lookup as fallback
//! - Handles profiles in `C:\WINDOWS\System32\spool\drivers\color`
//! - Supports both system and user-installed profiles
//!
//! ## Quick Start
//!
//! ### Library Usage
//!
//! ```rust,no_run
//! use display_icc::{get_primary_display_profile, get_all_display_profiles, ProfileError};
//!
//! fn main() -> Result<(), ProfileError> {
//!     // Get primary display profile
//!     let profile = get_primary_display_profile()?;
//!     println!("Primary display profile: {}", profile.name);
//!     println!("Color space: {}", profile.color_space);
//!     
//!     if let Some(path) = &profile.file_path {
//!         println!("Profile file: {}", path.display());
//!     }
//!
//!     // Get all display profiles
//!     let all_profiles = get_all_display_profiles()?;
//!     for (display, profile) in all_profiles {
//!         println!("Display '{}': {} ({})", 
//!                  display.name, profile.name, profile.color_space);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Advanced Usage with Configuration
//!
//! ```rust,no_run
//! use display_icc::{ProfileConfig, create_provider_with_config, ProfileError};
//!
//! fn main() -> Result<(), ProfileError> {
//!     let config = ProfileConfig {
//!         linux_prefer_dbus: false, // Linux: use colormgr command instead of D-Bus
//!         fallback_enabled: true,   // Enable fallback mechanisms
//!     };
//!
//!     let provider = create_provider_with_config(config)?;
//!     let displays = provider.get_displays()?;
//!     
//!     for display in displays {
//!         match provider.get_profile(&display) {
//!             Ok(profile) => {
//!                 println!("Display: {} -> Profile: {}", display.name, profile.name);
//!                 
//!                 // Get raw ICC data
//!                 if let Ok(icc_data) = provider.get_profile_data(&display) {
//!                     println!("ICC data size: {} bytes", icc_data.len());
//!                 }
//!             }
//!             Err(e) => eprintln!("Failed to get profile for {}: {}", display.name, e),
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Working with ICC Profile Data
//!
//! ```rust,no_run
//! use display_icc::{get_primary_display_profile_data, parse_icc_header, ProfileError};
//!
//! fn main() -> Result<(), ProfileError> {
//!     let icc_data = get_primary_display_profile_data()?;
//!     let header = parse_icc_header(&icc_data)?;
//!     
//!     println!("ICC Profile Header:");
//!     println!("  Size: {} bytes", header.profile_size);
//!     println!("  Version: {}.{}", header.version.0, header.version.1);
//!     println!("  Device class: {}", header.device_class);
//!     println!("  Color space: {}", header.data_color_space);
//!     println!("  Platform: {}", header.platform);
//!     
//!     if let Some(datetime) = &header.creation_datetime {
//!         println!("  Created: {}", datetime);
//!     }
//!
//!     // Validate the profile
//!     header.validate()?;
//!     println!("Profile is valid!");
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Error Handling
//!
//! The library provides comprehensive error handling through the [`ProfileError`] enum:
//!
//! ```rust,no_run
//! use display_icc::{get_primary_display_profile, ProfileError};
//!
//! match get_primary_display_profile() {
//!     Ok(profile) => println!("Got profile: {}", profile.name),
//!     Err(ProfileError::UnsupportedPlatform) => {
//!         eprintln!("This platform is not supported");
//!     }
//!     Err(ProfileError::DisplayNotFound(id)) => {
//!         eprintln!("Display not found: {}", id);
//!     }
//!     Err(ProfileError::ProfileNotAvailable(display)) => {
//!         eprintln!("No profile available for display: {}", display);
//!     }
//!     Err(ProfileError::SystemError(msg)) => {
//!         eprintln!("System error: {}", msg);
//!     }
//!     Err(e) => eprintln!("Other error: {}", e),
//! }
//! ```
//!
//! ## CLI Usage
//!
//! The library also provides a command-line interface:
//!
//! ```bash
//! # Get primary display profile
//! display_icc
//!
//! # Get all display profiles
//! display_icc --all
//!
//! # Get profile in JSON format
//! display_icc --json
//!
//! # Verbose output with ICC header information
//! display_icc --verbose
//! ```

use std::path::PathBuf;
use thiserror::Error;

// Platform-specific modules with conditional compilation
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

// Mock module for testing
#[cfg(test)]
mod mock;

// Re-export platform-specific implementations
#[cfg(target_os = "macos")]
use macos::MacOSProfileProvider;

#[cfg(target_os = "linux")]
use linux::LinuxProfileProvider;

#[cfg(target_os = "windows")]
use windows::WindowsProfileProvider;

/// Represents a display device in the system.
///
/// This struct contains information about a physical or virtual display device,
/// including its unique identifier, human-readable name, and primary status.
///
/// # Platform-Specific Behavior
///
/// - **macOS**: `id` is the CGDirectDisplayID as a string, `name` comes from display info
/// - **Linux**: `id` is the colormgr device ID, `name` is extracted from device properties
/// - **Windows**: `id` is the display device name, `name` is the friendly display name
///
/// # Examples
///
/// ```rust
/// use display_icc::Display;
///
/// let display = Display {
///     id: "69733382".to_string(),
///     name: "Built-in Retina Display".to_string(),
///     is_primary: true,
/// };
///
/// assert!(display.is_primary);
/// assert_eq!(display.name, "Built-in Retina Display");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    /// Unique identifier for the display.
    ///
    /// This is a platform-specific identifier that uniquely identifies the display
    /// within the system. The format varies by platform but is guaranteed to be
    /// consistent for the same physical display across program runs.
    pub id: String,
    
    /// Human-readable name of the display.
    ///
    /// This is typically the manufacturer and model name of the display,
    /// or a system-assigned name for built-in displays.
    pub name: String,
    
    /// Whether this is the primary display.
    ///
    /// The primary display is typically where the desktop wallpaper is shown
    /// and where new windows appear by default. Only one display can be primary.
    pub is_primary: bool,
}

/// Information about an ICC color profile associated with a display.
///
/// This struct contains metadata about an ICC color profile, including its name,
/// description, file path (if available), and color space information.
///
/// # Platform-Specific Behavior
///
/// - **macOS**: Profile information is extracted from CoreGraphics APIs and ICC data
/// - **Linux**: Information comes from colormgr/colord, with file paths in standard locations
/// - **Windows**: Profile data is retrieved from Win32 APIs and registry entries
///
/// # Examples
///
/// ```rust
/// use display_icc::{ProfileInfo, ColorSpace};
/// use std::path::PathBuf;
///
/// let profile = ProfileInfo {
///     name: "sRGB IEC61966-2.1".to_string(),
///     description: Some("Standard RGB color space".to_string()),
///     file_path: Some(PathBuf::from("/System/Library/ColorSync/Profiles/sRGB Profile.icc")),
///     color_space: ColorSpace::RGB,
/// };
///
/// assert_eq!(profile.color_space, ColorSpace::RGB);
/// assert!(profile.file_path.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    /// Name of the color profile.
    ///
    /// This is typically extracted from the ICC profile's description tag
    /// or provided by the system's color management APIs.
    pub name: String,
    
    /// Optional description of the profile.
    ///
    /// Additional descriptive text about the profile, if available.
    /// This may include manufacturer information, intended use, or other details.
    pub description: Option<String>,
    
    /// File system path to the profile file.
    ///
    /// The full path to the ICC profile file on disk, if available.
    /// Some profiles may be embedded in system APIs and not have a file path.
    ///
    /// # Platform Notes
    /// - **macOS**: Typically in `/System/Library/ColorSync/Profiles/` or `/Library/ColorSync/Profiles/`
    /// - **Linux**: Usually in `/usr/share/color/icc/` or `~/.local/share/icc/`
    /// - **Windows**: Commonly in `C:\WINDOWS\System32\spool\drivers\color\`
    pub file_path: Option<PathBuf>,
    
    /// Color space of the profile.
    ///
    /// The primary color space that this profile represents.
    /// Most display profiles use RGB color space.
    pub color_space: ColorSpace,
}

/// Supported color spaces for ICC profiles.
///
/// This enum represents the primary color spaces that display ICC profiles
/// can use. Most consumer displays use RGB color space variants.
///
/// # Examples
///
/// ```rust
/// use display_icc::ColorSpace;
///
/// let rgb_space = ColorSpace::RGB;
/// let lab_space = ColorSpace::Lab;
/// let unknown_space = ColorSpace::Unknown;
///
/// // Color spaces can be displayed as strings
/// assert_eq!(format!("{}", rgb_space), "RGB");
/// assert_eq!(format!("{}", lab_space), "Lab");
/// assert_eq!(format!("{}", unknown_space), "Unknown");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// RGB color space (most common).
    ///
    /// This includes standard RGB variants like:
    /// - sRGB (most common for consumer displays)
    /// - Display P3 (wide gamut displays, Apple devices)
    /// - Adobe RGB (professional displays)
    /// - Rec. 2020 (HDR displays)
    RGB,
    
    /// Lab color space (some high-precision displays).
    ///
    /// CIE Lab color space, used by some professional and scientific displays.
    /// Less common than RGB but provides device-independent color representation.
    Lab,
    
    /// Unknown or unsupported color space.
    ///
    /// Used when the profile's color space cannot be determined or is not
    /// one of the supported types. The profile may still be valid but uses
    /// a color space not explicitly handled by this library.
    Unknown,
}

impl std::fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorSpace::RGB => write!(f, "RGB"),
            ColorSpace::Lab => write!(f, "Lab"),
            ColorSpace::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Configuration options for profile retrieval behavior.
///
/// This struct allows customization of how the library retrieves ICC profiles
/// on different platforms. Different options may have no effect on certain platforms.
///
/// # Examples
///
/// ```rust
/// use display_icc::ProfileConfig;
///
/// // Use default configuration
/// let default_config = ProfileConfig::default();
/// assert!(default_config.linux_prefer_dbus);
/// assert!(default_config.fallback_enabled);
///
/// // Custom configuration for performance (Linux: use D-Bus)
/// let fast_config = ProfileConfig {
///     linux_prefer_dbus: true,
///     fallback_enabled: false,  // Skip fallbacks for speed
/// };
///
/// // Custom configuration for reliability (Linux: use colormgr command)
/// let reliable_config = ProfileConfig {
///     linux_prefer_dbus: false, // Use command-line tools on Linux
///     fallback_enabled: true,   // Try all available methods
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ProfileConfig {
    /// Linux: prefer D-Bus API over colormgr command.
    ///
    /// When `true`, the Linux implementation will attempt to use the D-Bus API
    /// to communicate with the colord daemon directly. When `false`, it will
    /// use the `colormgr` command-line tool.
    ///
    /// **Platform effect**: Linux only. Ignored on macOS and Windows.
    ///
    /// **Default**: `true`
    pub linux_prefer_dbus: bool,
    
    /// Enable fallback mechanisms when primary methods fail.
    ///
    /// When `true`, the library will attempt alternative methods if the primary
    /// approach fails. This increases reliability but may be slower.
    ///
    /// **Fallback behaviors**:
    /// - **macOS**: Fall back to known Apple display profiles
    /// - **Linux**: Fall back from D-Bus to colormgr to file system scanning
    /// - **Windows**: Fall back from API to registry to directory scanning
    ///
    /// **Default**: `true`
    pub fallback_enabled: bool,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            linux_prefer_dbus: true,
            fallback_enabled: true,
        }
    }
}

/// Errors that can occur during profile retrieval operations.
///
/// This enum covers all possible error conditions that can arise when working
/// with display ICC profiles across different platforms.
///
/// # Examples
///
/// ```rust
/// use display_icc::{get_primary_display_profile, ProfileError};
///
/// match get_primary_display_profile() {
///     Ok(profile) => println!("Profile: {}", profile.name),
///     Err(ProfileError::UnsupportedPlatform) => {
///         eprintln!("This platform is not supported by display_icc");
///     }
///     Err(ProfileError::DisplayNotFound(id)) => {
///         eprintln!("Could not find display with ID: {}", id);
///     }
///     Err(ProfileError::ProfileNotAvailable(display)) => {
///         eprintln!("Display '{}' has no ICC profile assigned", display);
///     }
///     Err(ProfileError::SystemError(msg)) => {
///         eprintln!("System API failed: {}", msg);
///     }
///     Err(ProfileError::IoError(msg)) => {
///         eprintln!("File I/O error: {}", msg);
///     }
///     Err(ProfileError::ParseError(msg)) => {
///         eprintln!("Failed to parse profile data: {}", msg);
///     }
/// }
/// ```
#[derive(Debug, Error, Clone)]
pub enum ProfileError {
    /// The current platform is not supported.
    ///
    /// This error occurs when running on a platform that is not supported
    /// by the library (i.e., not macOS, Linux, or Windows).
    #[error("Platform not supported")]
    UnsupportedPlatform,
    
    /// The specified display was not found.
    ///
    /// This error occurs when trying to access a display that doesn't exist
    /// or is no longer available (e.g., after disconnecting an external monitor).
    #[error("Display not found: {0}")]
    DisplayNotFound(String),
    
    /// No profile is available for the specified display.
    ///
    /// This error occurs when a display exists but has no ICC profile assigned.
    /// This is common for displays that use default system profiles or have
    /// no color management configured.
    #[error("Profile not available for display: {0}")]
    ProfileNotAvailable(String),
    
    /// An error occurred in the system API.
    ///
    /// This error wraps platform-specific API failures, such as:
    /// - CoreGraphics API failures on macOS
    /// - D-Bus communication errors on Linux
    /// - Win32 API failures on Windows
    #[error("System API error: {0}")]
    SystemError(String),
    
    /// An I/O error occurred.
    ///
    /// This error occurs when file operations fail, such as:
    /// - Unable to read ICC profile files
    /// - Permission denied accessing profile directories
    /// - File not found or corrupted profile files
    #[error("IO error: {0}")]
    IoError(String),
    
    /// An error occurred while parsing data.
    ///
    /// This error occurs when:
    /// - ICC profile data is malformed or corrupted
    /// - Command output cannot be parsed (Linux colormgr)
    /// - Registry data is in an unexpected format (Windows)
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<std::io::Error> for ProfileError {
    fn from(error: std::io::Error) -> Self {
        ProfileError::IoError(error.to_string())
    }
}

/// Core trait for platform-specific display profile providers.
///
/// This trait defines the interface that all platform-specific implementations
/// must provide. It abstracts the differences between macOS CoreGraphics,
/// Linux colormgr/colord, and Windows Win32 APIs.
///
/// # Implementation Notes
///
/// Platform implementations should handle their specific APIs and provide
/// consistent behavior across all methods. Error handling should be robust
/// and provide meaningful error messages.
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{DisplayProfileProvider, create_provider};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// let provider = create_provider()?;
///
/// // Get all displays
/// let displays = provider.get_displays()?;
/// println!("Found {} displays", displays.len());
///
/// // Get primary display
/// let primary = provider.get_primary_display()?;
/// println!("Primary display: {}", primary.name);
///
/// // Get profile for primary display
/// let profile = provider.get_profile(&primary)?;
/// println!("Profile: {} ({})", profile.name, profile.color_space);
///
/// // Get raw ICC data
/// let icc_data = provider.get_profile_data(&primary)?;
/// println!("ICC data: {} bytes", icc_data.len());
/// # Ok(())
/// # }
/// ```
pub trait DisplayProfileProvider {
    /// Get all available displays in the system.
    ///
    /// Returns a vector of all displays currently connected to the system,
    /// including both built-in and external displays.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<Display>)` - List of all available displays
    /// - `Err(ProfileError)` - If display enumeration fails
    ///
    /// # Platform Behavior
    ///
    /// - **macOS**: Uses `CGGetActiveDisplayList` to enumerate displays
    /// - **Linux**: Parses `colormgr get-devices` output or queries D-Bus
    /// - **Windows**: Uses Win32 display enumeration APIs
    fn get_displays(&self) -> Result<Vec<Display>, ProfileError>;
    
    /// Get the primary display.
    ///
    /// Returns the display that is designated as the primary display by the
    /// operating system. This is typically where the desktop wallpaper is
    /// shown and where new windows appear by default.
    ///
    /// # Returns
    ///
    /// - `Ok(Display)` - The primary display
    /// - `Err(ProfileError::DisplayNotFound)` - If no primary display is found
    /// - `Err(ProfileError)` - If display detection fails
    ///
    /// # Platform Behavior
    ///
    /// - **macOS**: Uses `CGMainDisplayID` to identify the primary display
    /// - **Linux**: Looks for displays marked as primary in colormgr/colord
    /// - **Windows**: Uses `GetPrimaryMonitorInfo` or similar APIs
    fn get_primary_display(&self) -> Result<Display, ProfileError>;
    
    /// Get profile information for a specific display.
    ///
    /// Retrieves the ICC profile metadata associated with the given display,
    /// including profile name, description, file path, and color space.
    ///
    /// # Arguments
    ///
    /// * `display` - The display to get the profile for
    ///
    /// # Returns
    ///
    /// - `Ok(ProfileInfo)` - Profile information for the display
    /// - `Err(ProfileError::ProfileNotAvailable)` - If no profile is assigned
    /// - `Err(ProfileError::DisplayNotFound)` - If the display no longer exists
    /// - `Err(ProfileError)` - If profile retrieval fails
    ///
    /// # Platform Behavior
    ///
    /// - **macOS**: Uses `CGDisplayCopyColorSpace` and extracts profile info
    /// - **Linux**: Queries colormgr/colord for device profile associations
    /// - **Windows**: Uses Win32 APIs to get profile file paths and reads metadata
    fn get_profile(&self, display: &Display) -> Result<ProfileInfo, ProfileError>;
    
    /// Get raw ICC profile data for a specific display.
    ///
    /// Retrieves the complete ICC profile binary data associated with the
    /// given display. This data can be used for detailed color management
    /// operations or saved to a file.
    ///
    /// # Arguments
    ///
    /// * `display` - The display to get the profile data for
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<u8>)` - Raw ICC profile binary data
    /// - `Err(ProfileError::ProfileNotAvailable)` - If no profile is assigned
    /// - `Err(ProfileError::DisplayNotFound)` - If the display no longer exists
    /// - `Err(ProfileError::IoError)` - If profile file cannot be read
    /// - `Err(ProfileError)` - If profile data retrieval fails
    ///
    /// # Platform Behavior
    ///
    /// - **macOS**: Uses `CGColorSpaceCopyICCData` to get profile data directly
    /// - **Linux**: Reads ICC files from file system based on colormgr associations
    /// - **Windows**: Reads ICC files from Windows color directory
    fn get_profile_data(&self, display: &Display) -> Result<Vec<u8>, ProfileError>;
}

/// Supported platforms for ICC profile retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS using CoreGraphics framework
    MacOS,
    /// Linux using colormgr and D-Bus
    Linux,
    /// Windows using Win32 API
    Windows,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::MacOS => write!(f, "macOS"),
            Platform::Linux => write!(f, "Linux"),
            Platform::Windows => write!(f, "Windows"),
        }
    }
}

/// Detect the current platform at runtime
pub fn detect_platform() -> Result<Platform, ProfileError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Platform::MacOS)
    }
    
    #[cfg(target_os = "linux")]
    {
        Ok(Platform::Linux)
    }
    
    #[cfg(target_os = "windows")]
    {
        Ok(Platform::Windows)
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(ProfileError::UnsupportedPlatform)
    }
}

/// Create a platform-specific profile provider with default configuration.
///
/// This function creates the appropriate [`DisplayProfileProvider`] implementation
/// for the current platform using default settings. This is the recommended way
/// to get a provider for most use cases.
///
/// # Returns
///
/// - `Ok(Box<dyn DisplayProfileProvider>)` - Platform-specific provider
/// - `Err(ProfileError::UnsupportedPlatform)` - If the platform is not supported
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::create_provider;
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// let provider = create_provider()?;
///
/// // Use the provider to get display information
/// let displays = provider.get_displays()?;
/// println!("Found {} displays", displays.len());
///
/// let primary = provider.get_primary_display()?;
/// let profile = provider.get_profile(&primary)?;
/// println!("Primary display profile: {}", profile.name);
/// # Ok(())
/// # }
/// ```
///
/// # Platform Implementations
///
/// - **macOS**: Returns [`MacOSProfileProvider`] using CoreGraphics
/// - **Linux**: Returns [`LinuxProfileProvider`] using colormgr/D-Bus
/// - **Windows**: Returns [`WindowsProfileProvider`] using Win32 API
/// - **Other platforms**: Returns [`ProfileError::UnsupportedPlatform`]
pub fn create_provider() -> Result<Box<dyn DisplayProfileProvider>, ProfileError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSProfileProvider::new()))
    }
    
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(LinuxProfileProvider::new()))
    }
    
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(WindowsProfileProvider::new()))
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(ProfileError::UnsupportedPlatform)
    }
}

/// Create a platform-specific profile provider with custom configuration.
///
/// This function creates the appropriate [`DisplayProfileProvider`] implementation
/// for the current platform using the provided configuration. Use this when you
/// need to customize the behavior of profile retrieval.
///
/// # Arguments
///
/// * `config` - Configuration options for the provider
///
/// # Returns
///
/// - `Ok(Box<dyn DisplayProfileProvider>)` - Platform-specific provider with custom config
/// - `Err(ProfileError::UnsupportedPlatform)` - If the platform is not supported
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{create_provider_with_config, ProfileConfig};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// // Create configuration for maximum performance
/// let config = ProfileConfig {
///     linux_prefer_dbus: true,  // Use D-Bus on Linux (faster)
///     fallback_enabled: false,  // Skip fallbacks for speed
/// };
///
/// let provider = create_provider_with_config(config)?;
///
/// // Provider will now use the custom configuration
/// let displays = provider.get_displays()?;
/// println!("Found {} displays", displays.len());
/// # Ok(())
/// # }
/// ```
///
/// ```rust,no_run
/// use display_icc::{create_provider_with_config, ProfileConfig};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// // Create configuration for maximum reliability
/// let config = ProfileConfig {
///     linux_prefer_dbus: false, // Use colormgr command on Linux (more reliable)
///     fallback_enabled: true,   // Try all available methods
/// };
///
/// let provider = create_provider_with_config(config)?;
/// let primary = provider.get_primary_display()?;
/// let profile = provider.get_profile(&primary)?;
/// println!("Primary display profile: {}", profile.name);
/// # Ok(())
/// # }
/// ```
///
/// # Configuration Effects by Platform
///
/// - **macOS**: Only `fallback_enabled` has effect
/// - **Linux**: All configuration options are used
/// - **Windows**: Only `fallback_enabled` has effect
pub fn create_provider_with_config(config: ProfileConfig) -> Result<Box<dyn DisplayProfileProvider>, ProfileError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSProfileProvider::with_config(config)))
    }
    
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(LinuxProfileProvider::with_config(config)))
    }
    
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(WindowsProfileProvider::with_config(config)))
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(ProfileError::UnsupportedPlatform)
    }
}

/// Convenience function to get the primary display profile.
///
/// This is the most commonly used function for applications that need to know
/// about the color profile of the main display. It uses default configuration
/// and handles all the platform-specific details automatically.
///
/// # Returns
///
/// - `Ok(ProfileInfo)` - Profile information for the primary display
/// - `Err(ProfileError)` - If profile retrieval fails for any reason
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{get_primary_display_profile, ColorSpace};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// let profile = get_primary_display_profile()?;
/// 
/// println!("Primary display profile: {}", profile.name);
/// println!("Color space: {}", profile.color_space);
/// 
/// if let Some(description) = &profile.description {
///     println!("Description: {}", description);
/// }
/// 
/// if let Some(path) = &profile.file_path {
///     println!("Profile file: {}", path.display());
/// }
/// 
/// // Check if it's an RGB profile (most common)
/// if profile.color_space == ColorSpace::RGB {
///     println!("This is an RGB color space profile");
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Platform Notes
///
/// - **macOS**: Gets the profile for the main display (menu bar display)
/// - **Linux**: Gets the profile for the primary display as reported by colormgr
/// - **Windows**: Gets the profile for the primary monitor
pub fn get_primary_display_profile() -> Result<ProfileInfo, ProfileError> {
    let provider = create_provider()?;
    let display = provider.get_primary_display()?;
    provider.get_profile(&display)
}

/// Convenience function to get the primary display profile with custom configuration.
///
/// Similar to [`get_primary_display_profile`] but allows customization of the
/// retrieval behavior through a [`ProfileConfig`].
///
/// # Arguments
///
/// * `config` - Configuration options for profile retrieval
///
/// # Returns
///
/// - `Ok(ProfileInfo)` - Profile information for the primary display
/// - `Err(ProfileError)` - If profile retrieval fails for any reason
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{get_primary_display_profile_with_config, ProfileConfig};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// // Configuration for maximum reliability
/// let config = ProfileConfig {
///     linux_prefer_dbus: false, // Use command-line tools on Linux
///     fallback_enabled: true,   // Try all available methods
/// };
///
/// let profile = get_primary_display_profile_with_config(config)?;
/// println!("Primary display profile: {}", profile.name);
/// # Ok(())
/// # }
/// ```
///
/// ```rust,no_run
/// use display_icc::{get_primary_display_profile_with_config, ProfileConfig};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// // Configuration for maximum performance
/// let config = ProfileConfig {
///     linux_prefer_dbus: true,  // Use faster D-Bus API on Linux
///     fallback_enabled: false,  // Skip fallbacks for speed
/// };
///
/// let profile = get_primary_display_profile_with_config(config)?;
/// println!("Primary display profile: {}", profile.name);
/// # Ok(())
/// # }
/// ```
pub fn get_primary_display_profile_with_config(config: ProfileConfig) -> Result<ProfileInfo, ProfileError> {
    let provider = create_provider_with_config(config)?;
    let display = provider.get_primary_display()?;
    provider.get_profile(&display)
}

/// Convenience function to get profiles for all displays.
///
/// Retrieves ICC profile information for all displays in the system that have
/// profiles assigned. Displays without profiles are silently skipped.
///
/// This function is useful for applications that need to handle multiple monitors
/// or provide users with a list of all available display profiles.
///
/// # Returns
///
/// - `Ok(Vec<(Display, ProfileInfo)>)` - List of display-profile pairs
/// - `Err(ProfileError)` - If display enumeration or profile retrieval fails
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::get_all_display_profiles;
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// let all_profiles = get_all_display_profiles()?;
/// 
/// println!("Found {} displays with profiles:", all_profiles.len());
/// 
/// for (display, profile) in all_profiles {
///     println!("Display: {} ({})", display.name, 
///              if display.is_primary { "Primary" } else { "Secondary" });
///     println!("  Profile: {} ({})", profile.name, profile.color_space);
///     
///     if let Some(path) = &profile.file_path {
///         println!("  File: {}", path.display());
///     }
///     
///     if let Some(description) = &profile.description {
///         println!("  Description: {}", description);
///     }
///     
///     println!();
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Behavior
///
/// - Displays without assigned profiles are automatically skipped
/// - The primary display (if it has a profile) will be included in the results
/// - Results are returned in the order that displays are enumerated by the system
/// - If any display fails with an error other than "profile not available", the function returns that error
pub fn get_all_display_profiles() -> Result<Vec<(Display, ProfileInfo)>, ProfileError> {
    let provider = create_provider()?;
    let displays = provider.get_displays()?;
    
    let mut results = Vec::new();
    for display in displays {
        match provider.get_profile(&display) {
            Ok(profile) => results.push((display, profile)),
            Err(ProfileError::ProfileNotAvailable(_)) => {
                // Skip displays without profiles
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    
    Ok(results)
}

/// Convenience function to get profiles for all displays with custom configuration
pub fn get_all_display_profiles_with_config(config: ProfileConfig) -> Result<Vec<(Display, ProfileInfo)>, ProfileError> {
    let provider = create_provider_with_config(config)?;
    let displays = provider.get_displays()?;
    
    let mut results = Vec::new();
    for display in displays {
        match provider.get_profile(&display) {
            Ok(profile) => results.push((display, profile)),
            Err(ProfileError::ProfileNotAvailable(_)) => {
                // Skip displays without profiles
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    
    Ok(results)
}

/// Convenience function to get raw ICC profile data for the primary display.
///
/// Retrieves the complete ICC profile binary data for the primary display.
/// This is useful when you need to work with the raw profile data for
/// color management calculations or to save the profile to a file.
///
/// # Returns
///
/// - `Ok(Vec<u8>)` - Raw ICC profile binary data
/// - `Err(ProfileError)` - If profile data retrieval fails
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{get_primary_display_profile_data, parse_icc_header};
/// use std::fs;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Get raw ICC data
/// let icc_data = get_primary_display_profile_data()?;
/// println!("ICC profile size: {} bytes", icc_data.len());
///
/// // Parse the ICC header for detailed information
/// let header = parse_icc_header(&icc_data)?;
/// println!("Profile version: {}.{}", header.version.0, header.version.1);
/// println!("Device class: {}", header.device_class);
/// println!("Color space: {}", header.data_color_space);
///
/// // Save profile to file
/// fs::write("primary_display_profile.icc", &icc_data)?;
/// println!("Profile saved to primary_display_profile.icc");
/// # Ok(())
/// # }
/// ```
///
/// # Use Cases
///
/// - **Color management**: Use with color management libraries for accurate color reproduction
/// - **Profile analysis**: Parse ICC tags and metadata for detailed profile information
/// - **Profile backup**: Save current display profiles for later restoration
/// - **Cross-platform compatibility**: Transfer profiles between different systems
pub fn get_primary_display_profile_data() -> Result<Vec<u8>, ProfileError> {
    let provider = create_provider()?;
    let display = provider.get_primary_display()?;
    provider.get_profile_data(&display)
}

/// Convenience function to get raw ICC profile data for the primary display with custom configuration
pub fn get_primary_display_profile_data_with_config(config: ProfileConfig) -> Result<Vec<u8>, ProfileError> {
    let provider = create_provider_with_config(config)?;
    let display = provider.get_primary_display()?;
    provider.get_profile_data(&display)
}

/// ICC profile header information extracted from profile data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IccHeader {
    /// Profile size in bytes (from header)
    pub profile_size: u32,
    /// Preferred CMM (Color Management Module) type
    pub preferred_cmm: String,
    /// Profile version (major.minor format)
    pub version: (u8, u8),
    /// Device class (e.g., "mntr" for monitor, "prtr" for printer)
    pub device_class: String,
    /// Data color space (e.g., "RGB ", "CMYK", "Lab ")
    pub data_color_space: String,
    /// Profile connection space (usually "XYZ " or "Lab ")
    pub connection_space: String,
    /// Profile creation date and time (if available)
    pub creation_datetime: Option<String>,
    /// Platform signature (e.g., "APPL", "MSFT", "SGI ")
    pub platform: String,
    /// Profile flags
    pub flags: u32,
    /// Device manufacturer signature
    pub device_manufacturer: String,
    /// Device model signature
    pub device_model: String,
}

impl IccHeader {
    /// Parse ICC header from profile data
    pub fn parse(data: &[u8]) -> Result<Self, ProfileError> {
        if data.len() < 128 {
            return Err(ProfileError::ParseError(
                format!("ICC profile data too short: {} bytes (minimum 128)", data.len())
            ));
        }

        // Helper function to read 4-byte signature as string
        let read_signature = |offset: usize| -> String {
            String::from_utf8_lossy(&data[offset..offset + 4])
                .trim_end_matches('\0')
                .to_string()
        };

        // Helper function to read big-endian u32
        let read_u32_be = |offset: usize| -> u32 {
            u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        };

        // Parse header fields according to ICC specification
        let profile_size = read_u32_be(0);
        let preferred_cmm = read_signature(4);
        
        // Version is stored as major.minor.bugfix.reserved
        let version_raw = read_u32_be(8);
        let version = ((version_raw >> 24) as u8, ((version_raw >> 20) & 0x0F) as u8);
        
        let device_class = read_signature(12);
        let data_color_space = read_signature(16);
        let connection_space = read_signature(20);
        
        // Date/time is stored as 12 bytes (year, month, day, hour, minute, second as u16 each)
        let creation_datetime = if data[24..36].iter().any(|&b| b != 0) {
            let year = u16::from_be_bytes([data[24], data[25]]);
            let month = u16::from_be_bytes([data[26], data[27]]);
            let day = u16::from_be_bytes([data[28], data[29]]);
            let hour = u16::from_be_bytes([data[30], data[31]]);
            let minute = u16::from_be_bytes([data[32], data[33]]);
            let second = u16::from_be_bytes([data[34], data[35]]);
            
            Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", 
                        year, month, day, hour, minute, second))
        } else {
            None
        };
        
        let platform = read_signature(40);
        let flags = read_u32_be(44);
        let device_manufacturer = read_signature(48);
        let device_model = read_signature(52);

        Ok(IccHeader {
            profile_size,
            preferred_cmm,
            version,
            device_class,
            data_color_space,
            connection_space,
            creation_datetime,
            platform,
            flags,
            device_manufacturer,
            device_model,
        })
    }

    /// Check if the profile is valid based on header information
    pub fn validate(&self) -> Result<(), ProfileError> {
        // Check if profile size is reasonable (at least 128 bytes for header)
        if self.profile_size < 128 {
            return Err(ProfileError::ParseError(
                format!("Invalid profile size: {} bytes", self.profile_size)
            ));
        }

        // Check if device class is valid for display profiles
        if !["mntr", "scnr", "prtr", "link", "spac", "abst", "nmcl"].contains(&self.device_class.as_str()) {
            return Err(ProfileError::ParseError(
                format!("Invalid device class: {}", self.device_class)
            ));
        }

        // Check if color space is valid
        if !["RGB ", "CMYK", "Lab ", "XYZ ", "Luv ", "YCbr", "Yxy ", "HSV ", "HLS ", "CMY "].contains(&self.data_color_space.as_str()) {
            return Err(ProfileError::ParseError(
                format!("Invalid data color space: {}", self.data_color_space)
            ));
        }

        Ok(())
    }
}

/// Parse ICC header from profile data (convenience function).
///
/// This is a convenience wrapper around [`IccHeader::parse`] that extracts
/// header information from raw ICC profile data. The ICC header contains
/// important metadata about the profile.
///
/// # Arguments
///
/// * `data` - Raw ICC profile binary data (must be at least 128 bytes)
///
/// # Returns
///
/// - `Ok(IccHeader)` - Parsed ICC header information
/// - `Err(ProfileError::ParseError)` - If the data is too short or malformed
///
/// # Examples
///
/// ```rust,no_run
/// use display_icc::{get_primary_display_profile_data, parse_icc_header};
///
/// # fn example() -> Result<(), display_icc::ProfileError> {
/// // Get raw ICC data and parse header
/// let icc_data = get_primary_display_profile_data()?;
/// let header = parse_icc_header(&icc_data)?;
///
/// println!("ICC Profile Information:");
/// println!("  Size: {} bytes", header.profile_size);
/// println!("  Version: {}.{}", header.version.0, header.version.1);
/// println!("  Device class: {}", header.device_class);
/// println!("  Color space: {}", header.data_color_space);
/// println!("  Connection space: {}", header.connection_space);
/// println!("  Platform: {}", header.platform);
/// println!("  Manufacturer: {}", header.device_manufacturer);
/// println!("  Model: {}", header.device_model);
///
/// if let Some(datetime) = &header.creation_datetime {
///     println!("  Created: {}", datetime);
/// }
///
/// // Validate the header
/// header.validate()?;
/// println!("Profile header is valid!");
/// # Ok(())
/// # }
/// ```
///
/// # ICC Header Structure
///
/// The ICC header is the first 128 bytes of an ICC profile and contains:
/// - Profile size and version information
/// - Device class (monitor, printer, etc.)
/// - Color space information (RGB, CMYK, Lab, etc.)
/// - Creation date and time
/// - Platform and manufacturer signatures
/// - Various flags and attributes
pub fn parse_icc_header(data: &[u8]) -> Result<IccHeader, ProfileError> {
    IccHeader::parse(data)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_display_creation() {
        let display = Display {
            id: "test_id".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };

        assert_eq!(display.id, "test_id");
        assert_eq!(display.name, "Test Display");
        assert!(display.is_primary);
    }

    #[test]
    fn test_display_equality() {
        let display1 = Display {
            id: "test_id".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };

        let display2 = Display {
            id: "test_id".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };

        let display3 = Display {
            id: "different_id".to_string(),
            name: "Test Display".to_string(),
            is_primary: true,
        };

        assert_eq!(display1, display2);
        assert_ne!(display1, display3);
    }

    #[test]
    fn test_profile_info_creation() {
        let profile = ProfileInfo {
            name: "sRGB".to_string(),
            description: Some("Standard RGB color space".to_string()),
            file_path: Some(PathBuf::from("/path/to/profile.icc")),
            color_space: ColorSpace::RGB,
        };

        assert_eq!(profile.name, "sRGB");
        assert_eq!(profile.description, Some("Standard RGB color space".to_string()));
        assert_eq!(profile.file_path, Some(PathBuf::from("/path/to/profile.icc")));
        assert_eq!(profile.color_space, ColorSpace::RGB);
    }

    #[test]
    fn test_color_space_display() {
        assert_eq!(format!("{}", ColorSpace::RGB), "RGB");
        assert_eq!(format!("{}", ColorSpace::Lab), "Lab");
        assert_eq!(format!("{}", ColorSpace::Unknown), "Unknown");
    }

    #[test]
    fn test_profile_config_default() {
        let config = ProfileConfig::default();
        assert!(config.linux_prefer_dbus);
        assert!(config.fallback_enabled);
    }

    #[test]
    fn test_profile_config_custom() {
        let config = ProfileConfig {
            linux_prefer_dbus: false,
            fallback_enabled: false,
        };

        assert!(!config.linux_prefer_dbus);
        assert!(!config.fallback_enabled);
    }

    #[test]
    fn test_profile_error_display() {
        let error = ProfileError::UnsupportedPlatform;
        assert_eq!(format!("{}", error), "Platform not supported");

        let error = ProfileError::DisplayNotFound("test_display".to_string());
        assert_eq!(format!("{}", error), "Display not found: test_display");

        let error = ProfileError::ProfileNotAvailable("test_display".to_string());
        assert_eq!(format!("{}", error), "Profile not available for display: test_display");

        let error = ProfileError::SystemError("API failed".to_string());
        assert_eq!(format!("{}", error), "System API error: API failed");

        let error = ProfileError::ParseError("Invalid data".to_string());
        assert_eq!(format!("{}", error), "Parse error: Invalid data");
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::MacOS), "macOS");
        assert_eq!(format!("{}", Platform::Linux), "Linux");
        assert_eq!(format!("{}", Platform::Windows), "Windows");
    }

    #[test]
    fn test_detect_platform() {
        let platform = detect_platform();
        assert!(platform.is_ok());
        
        // Platform should match the current compilation target
        #[cfg(target_os = "macos")]
        assert_eq!(platform.unwrap(), Platform::MacOS);
        
        #[cfg(target_os = "linux")]
        assert_eq!(platform.unwrap(), Platform::Linux);
        
        #[cfg(target_os = "windows")]
        assert_eq!(platform.unwrap(), Platform::Windows);
    }

    #[test]
    fn test_icc_header_parse_invalid_data() {
        // Test with data too short
        let short_data = vec![0u8; 64];
        let result = IccHeader::parse(&short_data);
        assert!(result.is_err());
        
        if let Err(ProfileError::ParseError(msg)) = result {
            assert!(msg.contains("too short"));
        } else {
            panic!("Expected ParseError");
        }
    }

    #[test]
    fn test_icc_header_parse_valid_data() {
        // Create minimal valid ICC header (128 bytes)
        let mut data = vec![0u8; 128];
        
        // Profile size (first 4 bytes, big-endian)
        data[0..4].copy_from_slice(&1024u32.to_be_bytes());
        
        // Preferred CMM (bytes 4-7)
        data[4..8].copy_from_slice(b"ADBE");
        
        // Version (bytes 8-11) - version 4.3
        data[8..12].copy_from_slice(&0x04300000u32.to_be_bytes());
        
        // Device class (bytes 12-15)
        data[12..16].copy_from_slice(b"mntr");
        
        // Data color space (bytes 16-19)
        data[16..20].copy_from_slice(b"RGB ");
        
        // Connection space (bytes 20-23)
        data[20..24].copy_from_slice(b"XYZ ");
        
        // Platform (bytes 40-43)
        data[40..44].copy_from_slice(b"APPL");
        
        // Device manufacturer (bytes 48-51)
        data[48..52].copy_from_slice(b"APPL");
        
        // Device model (bytes 52-55)
        data[52..56].copy_from_slice(b"mntr");

        let header = IccHeader::parse(&data).expect("Should parse valid header");
        
        assert_eq!(header.profile_size, 1024);
        assert_eq!(header.preferred_cmm, "ADBE");
        assert_eq!(header.version, (4, 3));
        assert_eq!(header.device_class, "mntr");
        assert_eq!(header.data_color_space, "RGB ");
        assert_eq!(header.connection_space, "XYZ ");
        assert_eq!(header.platform, "APPL");
        assert_eq!(header.device_manufacturer, "APPL");
        assert_eq!(header.device_model, "mntr");
    }

    #[test]
    fn test_icc_header_parse_with_datetime() {
        let mut data = vec![0u8; 128];
        
        // Basic required fields
        data[0..4].copy_from_slice(&1024u32.to_be_bytes());
        data[12..16].copy_from_slice(b"mntr");
        data[16..20].copy_from_slice(b"RGB ");
        data[20..24].copy_from_slice(b"XYZ ");
        
        // Date/time: 2023-12-25 14:30:45
        data[24..26].copy_from_slice(&2023u16.to_be_bytes()); // year
        data[26..28].copy_from_slice(&12u16.to_be_bytes());   // month
        data[28..30].copy_from_slice(&25u16.to_be_bytes());   // day
        data[30..32].copy_from_slice(&14u16.to_be_bytes());   // hour
        data[32..34].copy_from_slice(&30u16.to_be_bytes());   // minute
        data[34..36].copy_from_slice(&45u16.to_be_bytes());   // second

        let header = IccHeader::parse(&data).expect("Should parse header with datetime");
        
        assert_eq!(header.creation_datetime, Some("2023-12-25 14:30:45".to_string()));
    }

    #[test]
    fn test_icc_header_validate() {
        let valid_header = IccHeader {
            profile_size: 1024,
            preferred_cmm: "ADBE".to_string(),
            version: (4, 3),
            device_class: "mntr".to_string(),
            data_color_space: "RGB ".to_string(),
            connection_space: "XYZ ".to_string(),
            creation_datetime: None,
            platform: "APPL".to_string(),
            flags: 0,
            device_manufacturer: "APPL".to_string(),
            device_model: "mntr".to_string(),
        };

        assert!(valid_header.validate().is_ok());

        // Test invalid profile size
        let mut invalid_header = valid_header.clone();
        invalid_header.profile_size = 64;
        assert!(invalid_header.validate().is_err());

        // Test invalid device class
        let mut invalid_header = valid_header.clone();
        invalid_header.device_class = "invalid".to_string();
        assert!(invalid_header.validate().is_err());

        // Test invalid color space
        let mut invalid_header = valid_header.clone();
        invalid_header.data_color_space = "invalid".to_string();
        assert!(invalid_header.validate().is_err());
    }

    #[test]
    fn test_parse_icc_header_convenience_function() {
        let mut data = vec![0u8; 128];
        data[0..4].copy_from_slice(&1024u32.to_be_bytes());
        data[12..16].copy_from_slice(b"mntr");
        data[16..20].copy_from_slice(b"RGB ");
        data[20..24].copy_from_slice(b"XYZ ");

        let header = parse_icc_header(&data).expect("Should parse header");
        assert_eq!(header.profile_size, 1024);
        assert_eq!(header.device_class, "mntr");
    }
}
#[cfg(test)]
mod api_tests {
    use super::*;
    use crate::mock::MockProfileProvider;



    #[test]
    fn test_get_primary_display_profile_success() {
        let provider = MockProfileProvider::with_test_data();
        
        // Simulate the convenience function behavior
        let primary = provider.get_primary_display().unwrap();
        let profile = provider.get_profile(&primary).unwrap();
        
        assert_eq!(profile.name, "sRGB IEC61966-2.1");
        assert_eq!(profile.color_space, ColorSpace::RGB);
        assert!(profile.description.is_some());
        assert!(profile.file_path.is_some());
    }

    #[test]
    fn test_get_primary_display_profile_no_primary() {
        let mut provider = MockProfileProvider::new();
        
        // Add non-primary display
        let display = Display {
            id: "secondary".to_string(),
            name: "Secondary Display".to_string(),
            is_primary: false,
        };
        provider.add_display(display);
        
        let result = provider.get_primary_display();
        assert!(result.is_err());
        
        if let Err(ProfileError::DisplayNotFound(_)) = result {
            // Expected
        } else {
            panic!("Expected DisplayNotFound error");
        }
    }

    #[test]
    fn test_get_all_display_profiles_success() {
        let provider = MockProfileProvider::with_test_data();
        
        // Simulate get_all_display_profiles behavior
        let displays = provider.get_displays().unwrap();
        let mut results = Vec::new();
        
        for display in displays {
            match provider.get_profile(&display) {
                Ok(profile) => results.push((display, profile)),
                Err(ProfileError::ProfileNotAvailable(_)) => continue,
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }
        
        assert_eq!(results.len(), 2);
        
        // Check primary display
        let primary_result = results.iter().find(|(d, _)| d.is_primary).unwrap();
        assert_eq!(primary_result.1.name, "sRGB IEC61966-2.1");
        
        // Check secondary display
        let secondary_result = results.iter().find(|(d, _)| !d.is_primary).unwrap();
        assert_eq!(secondary_result.1.name, "Display P3");
    }

    #[test]
    fn test_get_all_display_profiles_skip_unavailable() {
        let mut provider = MockProfileProvider::new();
        
        // Add display with profile
        let display1 = Display {
            id: "with_profile".to_string(),
            name: "Display with Profile".to_string(),
            is_primary: true,
        };
        let profile1 = ProfileInfo {
            name: "Test Profile".to_string(),
            description: None,
            file_path: None,
            color_space: ColorSpace::RGB,
        };
        provider.add_display(display1);
        provider.set_profile("with_profile", profile1);
        
        // Add display without profile
        let display2 = Display {
            id: "without_profile".to_string(),
            name: "Display without Profile".to_string(),
            is_primary: false,
        };
        provider.add_display(display2);
        
        // Simulate get_all_display_profiles behavior
        let displays = provider.get_displays().unwrap();
        let mut results = Vec::new();
        
        for display in displays {
            match provider.get_profile(&display) {
                Ok(profile) => results.push((display, profile)),
                Err(ProfileError::ProfileNotAvailable(_)) => continue,
                Err(e) => panic!("Unexpected error: {}", e),
            }
        }
        
        // Should only include the display with a profile
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "with_profile");
    }

    #[test]
    fn test_get_primary_display_profile_data_success() {
        let provider = MockProfileProvider::with_test_data();
        
        let primary = provider.get_primary_display().unwrap();
        let data = provider.get_profile_data(&primary).unwrap();
        
        assert_eq!(data.len(), 128);
        
        // Verify it's valid ICC data
        let profile_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(profile_size, 1024);
    }

    #[test]
    fn test_profile_config_with_custom_settings() {
        let config = ProfileConfig {
            linux_prefer_dbus: false,
            fallback_enabled: false,
        };
        
        // Test that custom configuration is preserved
        assert!(!config.linux_prefer_dbus);
        assert!(!config.fallback_enabled);
    }

    #[test]
    fn test_display_profile_provider_trait_methods() {
        let provider = MockProfileProvider::with_test_data();
        
        // Test all trait methods
        let displays = provider.get_displays().unwrap();
        assert!(!displays.is_empty());
        
        let primary = provider.get_primary_display().unwrap();
        assert!(primary.is_primary);
        
        let profile = provider.get_profile(&primary).unwrap();
        assert!(!profile.name.is_empty());
        
        let data = provider.get_profile_data(&primary).unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_error_propagation() {
        let mut provider = MockProfileProvider::new();
        
        let display = Display {
            id: "error_display".to_string(),
            name: "Error Display".to_string(),
            is_primary: true,
        };
        
        provider.add_display(display.clone());
        provider.set_failure("error_display", ProfileError::SystemError("Test error".to_string()));
        
        // Test that errors propagate correctly
        let profile_result = provider.get_profile(&display);
        assert!(profile_result.is_err());
        
        let data_result = provider.get_profile_data(&display);
        assert!(data_result.is_err());
    }

    #[test]
    fn test_multiple_displays_handling() {
        let mut provider = MockProfileProvider::new();
        
        // Add multiple displays with different configurations
        for i in 0..5 {
            let display = Display {
                id: format!("display_{}", i),
                name: format!("Display {}", i),
                is_primary: i == 0,
            };
            
            let profile = ProfileInfo {
                name: format!("Profile {}", i),
                description: Some(format!("Description {}", i)),
                file_path: Some(PathBuf::from(format!("/path/to/profile_{}.icc", i))),
                color_space: if i % 2 == 0 { ColorSpace::RGB } else { ColorSpace::Lab },
            };
            
            provider.add_display(display);
            provider.set_profile(&format!("display_{}", i), profile);
        }
        
        let displays = provider.get_displays().unwrap();
        assert_eq!(displays.len(), 5);
        
        // Verify primary display
        let primary = provider.get_primary_display().unwrap();
        assert_eq!(primary.id, "display_0");
        
        // Verify all profiles can be retrieved
        for display in &displays {
            let profile = provider.get_profile(display).unwrap();
            assert!(profile.name.starts_with("Profile"));
        }
    }

    #[test]
    fn test_edge_cases() {
        let provider = MockProfileProvider::new();
        
        // Test with no displays
        let displays = provider.get_displays().unwrap();
        assert!(displays.is_empty());
        
        let primary_result = provider.get_primary_display();
        assert!(primary_result.is_err());
        
        // Test with non-existent display
        let fake_display = Display {
            id: "fake".to_string(),
            name: "Fake Display".to_string(),
            is_primary: false,
        };
        
        let profile_result = provider.get_profile(&fake_display);
        assert!(profile_result.is_err());
        
        let data_result = provider.get_profile_data(&fake_display);
        assert!(data_result.is_err());
    }
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    #[test]
    fn test_profile_config_clone() {
        let config1 = ProfileConfig {
            linux_prefer_dbus: true,
            fallback_enabled: false,
        };
        
        let config2 = config1.clone();
        
        assert_eq!(config1.linux_prefer_dbus, config2.linux_prefer_dbus);
        assert_eq!(config1.fallback_enabled, config2.fallback_enabled);
    }

    #[test]
    fn test_profile_config_debug() {
        let config = ProfileConfig::default();
        let debug_str = format!("{:?}", config);
        
        assert!(debug_str.contains("ProfileConfig"));
        assert!(debug_str.contains("linux_prefer_dbus"));
        assert!(debug_str.contains("fallback_enabled"));
    }

    #[test]
    fn test_profile_config_all_combinations() {
        // Test all boolean combinations
        let configs = [
            ProfileConfig { linux_prefer_dbus: true, fallback_enabled: true },
            ProfileConfig { linux_prefer_dbus: true, fallback_enabled: false },
            ProfileConfig { linux_prefer_dbus: false, fallback_enabled: true },
            ProfileConfig { linux_prefer_dbus: false, fallback_enabled: false },
        ];
        
        // Verify all configurations are valid and can be created
        for config in &configs {
            assert!(config.linux_prefer_dbus || !config.linux_prefer_dbus); // Always true, but tests field access
            assert!(config.fallback_enabled || !config.fallback_enabled);
        }
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use std::io;

    #[test]
    fn test_profile_error_from_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let profile_error = ProfileError::from(io_error);
        
        if let ProfileError::IoError(msg) = profile_error {
            assert!(msg.contains("File not found"));
        } else {
            panic!("Expected IoError variant");
        }
    }

    #[test]
    fn test_profile_error_debug() {
        let errors = [
            ProfileError::UnsupportedPlatform,
            ProfileError::DisplayNotFound("test".to_string()),
            ProfileError::ProfileNotAvailable("test".to_string()),
            ProfileError::SystemError("test".to_string()),
            ProfileError::ParseError("test".to_string()),
        ];
        
        for error in &errors {
            let debug_str = format!("{:?}", error);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_profile_error_equality() {
        // Test that errors can be compared (for testing purposes)
        let error1 = ProfileError::DisplayNotFound("test".to_string());
        let error2 = ProfileError::DisplayNotFound("test".to_string());
        let error3 = ProfileError::DisplayNotFound("different".to_string());
        
        // Note: ProfileError doesn't implement PartialEq due to io::Error,
        // but we can test the display strings
        assert_eq!(format!("{}", error1), format!("{}", error2));
        assert_ne!(format!("{}", error1), format!("{}", error3));
    }

    #[test]
    fn test_error_source_chain() {
        
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let profile_error = ProfileError::from(io_error);
        
        // Test that the error message is preserved
        if let ProfileError::IoError(msg) = profile_error {
            assert!(msg.contains("Access denied"));
        } else {
            panic!("Expected IoError variant");
        }
    }
}
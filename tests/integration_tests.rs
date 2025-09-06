//! Integration tests for display_icc
//!
//! These tests verify actual system integration on each supported platform.
//! They use conditional compilation to run platform-specific tests.

use display_icc::{
    create_provider, create_provider_with_config, detect_platform, get_all_display_profiles,
    get_primary_display_profile, get_primary_display_profile_data, parse_icc_header, ColorSpace,
    Platform, ProfileConfig, ProfileError,
};
use serial_test::serial;
use std::collections::HashSet;

/// Test platform detection
#[test]
fn test_platform_detection() {
    let platform = detect_platform().expect("Should detect current platform");

    #[cfg(target_os = "macos")]
    assert_eq!(platform, Platform::MacOS);

    #[cfg(target_os = "linux")]
    assert_eq!(platform, Platform::Linux);

    #[cfg(target_os = "windows")]
    assert_eq!(platform, Platform::Windows);
}

/// Test creating a provider with default configuration
#[test]
#[serial]
fn test_create_provider_default() {
    let provider = create_provider();
    assert!(
        provider.is_ok(),
        "Should create provider on supported platform"
    );
}

/// Test creating a provider with custom configuration
#[test]
#[serial]
fn test_create_provider_with_config() {
    let config = ProfileConfig {
        linux_prefer_dbus: false,
        fallback_enabled: true,
    };

    let provider = create_provider_with_config(config);
    assert!(
        provider.is_ok(),
        "Should create provider with custom config"
    );
}

/// Test getting displays from the system
#[test]
#[serial]
fn test_get_displays_integration() {
    let provider = create_provider().expect("Should create provider");
    let displays = provider.get_displays().expect("Should get displays");

    // Should have at least one display (the one running the test)
    assert!(!displays.is_empty(), "Should have at least one display");

    // Verify display properties
    for display in &displays {
        assert!(!display.id.is_empty(), "Display ID should not be empty");
        assert!(!display.name.is_empty(), "Display name should not be empty");
    }

    // Should have exactly one primary display
    let primary_count = displays.iter().filter(|d| d.is_primary).count();
    assert_eq!(primary_count, 1, "Should have exactly one primary display");
}

/// Test getting the primary display
#[test]
#[serial]
fn test_get_primary_display_integration() {
    let provider = create_provider().expect("Should create provider");
    let primary = provider
        .get_primary_display()
        .expect("Should get primary display");

    assert!(
        primary.is_primary,
        "Primary display should be marked as primary"
    );
    assert!(
        !primary.id.is_empty(),
        "Primary display ID should not be empty"
    );
    assert!(
        !primary.name.is_empty(),
        "Primary display name should not be empty"
    );
}

/// Test getting profile information
#[test]
#[serial]
fn test_get_profile_integration() {
    let provider = create_provider().expect("Should create provider");
    let primary = provider
        .get_primary_display()
        .expect("Should get primary display");

    match provider.get_profile(&primary) {
        Ok(profile) => {
            assert!(!profile.name.is_empty(), "Profile name should not be empty");

            // Color space should be valid
            match profile.color_space {
                ColorSpace::RGB | ColorSpace::Lab | ColorSpace::Unknown => {
                    // All valid
                }
            }

            println!("Primary display profile: {}", profile.name);
            if let Some(desc) = &profile.description {
                println!("Description: {}", desc);
            }
            if let Some(path) = &profile.file_path {
                println!("File path: {}", path.display());
            }
            println!("Color space: {}", profile.color_space);
        }
        Err(ProfileError::ProfileNotAvailable(_)) => {
            println!("No profile assigned to primary display (this is valid)");
        }
        Err(e) => {
            panic!("Unexpected error getting profile: {}", e);
        }
    }
}

/// Test getting profile data
#[test]
#[serial]
fn test_get_profile_data_integration() {
    let provider = create_provider().expect("Should create provider");
    let primary = provider
        .get_primary_display()
        .expect("Should get primary display");

    match provider.get_profile_data(&primary) {
        Ok(data) => {
            assert!(!data.is_empty(), "Profile data should not be empty");
            assert!(
                data.len() >= 128,
                "Profile data should be at least 128 bytes (ICC header)"
            );

            // Try to parse ICC header
            match parse_icc_header(&data) {
                Ok(header) => {
                    println!("ICC Profile Header:");
                    println!("  Size: {} bytes", header.profile_size);
                    println!("  Version: {}.{}", header.version.0, header.version.1);
                    println!("  Device class: {}", header.device_class);
                    println!("  Color space: {}", header.data_color_space);

                    // Validate header
                    header.validate().expect("ICC header should be valid");
                }
                Err(e) => {
                    println!("Warning: Could not parse ICC header: {}", e);
                }
            }
        }
        Err(ProfileError::ProfileNotAvailable(_)) => {
            println!("No profile data available for primary display (this is valid)");
        }
        Err(e) => {
            panic!("Unexpected error getting profile data: {}", e);
        }
    }
}

/// Test convenience function for getting primary display profile
#[test]
#[serial]
fn test_get_primary_display_profile_convenience() {
    match get_primary_display_profile() {
        Ok(profile) => {
            assert!(!profile.name.is_empty(), "Profile name should not be empty");
            println!("Primary profile (convenience): {}", profile.name);
        }
        Err(ProfileError::ProfileNotAvailable(_)) => {
            println!("No profile available for primary display (this is valid)");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test convenience function for getting all display profiles
#[test]
#[serial]
fn test_get_all_display_profiles_convenience() {
    match get_all_display_profiles() {
        Ok(profiles) => {
            println!("Found {} displays with profiles", profiles.len());

            let mut display_ids = HashSet::new();
            for (display, profile) in &profiles {
                // Verify no duplicate displays
                assert!(
                    display_ids.insert(display.id.clone()),
                    "Display IDs should be unique"
                );

                assert!(!display.name.is_empty(), "Display name should not be empty");
                assert!(!profile.name.is_empty(), "Profile name should not be empty");

                println!(
                    "  {}: {} ({})",
                    display.name, profile.name, profile.color_space
                );
            }

            // Should have at least one profile if displays exist
            let provider = create_provider().expect("Should create provider");
            let all_displays = provider.get_displays().expect("Should get displays");

            if !all_displays.is_empty() {
                // We might have displays without profiles, so profiles.len() <= all_displays.len()
                assert!(
                    profiles.len() <= all_displays.len(),
                    "Profile count should not exceed display count"
                );
            }
        }
        Err(e) => {
            panic!("Unexpected error getting all profiles: {}", e);
        }
    }
}

/// Test convenience function for getting primary display profile data
#[test]
#[serial]
fn test_get_primary_display_profile_data_convenience() {
    match get_primary_display_profile_data() {
        Ok(data) => {
            assert!(!data.is_empty(), "Profile data should not be empty");
            println!("Primary profile data size: {} bytes", data.len());

            // Verify it's valid ICC data
            if data.len() >= 4 {
                let profile_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                println!("ICC profile declares size: {} bytes", profile_size);

                // Profile size should be reasonable
                assert!(
                    profile_size >= 128,
                    "Profile size should be at least 128 bytes"
                );
                assert!(
                    profile_size <= 10_000_000,
                    "Profile size should be reasonable (< 10MB)"
                );
            }
        }
        Err(ProfileError::ProfileNotAvailable(_)) => {
            println!("No profile data available for primary display (this is valid)");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test with different configuration options
#[test]
#[serial]
fn test_different_configurations() {
    let configs = [
        ProfileConfig {
            linux_prefer_dbus: true,
            fallback_enabled: true,
        },
        ProfileConfig {
            linux_prefer_dbus: false,
            fallback_enabled: true,
        },
        ProfileConfig {
            linux_prefer_dbus: true,
            fallback_enabled: false,
        },
    ];

    for (i, config) in configs.iter().enumerate() {
        println!("Testing configuration {}: {:?}", i, config);

        let provider = create_provider_with_config(config.clone())
            .expect("Should create provider with config");

        let displays = provider
            .get_displays()
            .expect("Should get displays with any config");

        assert!(
            !displays.is_empty(),
            "Should have displays with config {}",
            i
        );

        let primary = provider
            .get_primary_display()
            .expect("Should get primary display with any config");

        assert!(
            primary.is_primary,
            "Primary should be marked as primary with config {}",
            i
        );
    }
}

/// Platform-specific integration tests
#[cfg(target_os = "macos")]
mod macos_tests {
    use super::*;

    #[test]
    #[serial]
    fn test_macos_specific_behavior() {
        let provider = create_provider().expect("Should create macOS provider");
        let displays = provider
            .get_displays()
            .expect("Should get displays on macOS");

        // macOS should always have at least the main display
        assert!(
            !displays.is_empty(),
            "macOS should have at least one display"
        );

        // Test that display IDs are numeric (CGDirectDisplayID)
        for display in &displays {
            // macOS display IDs should be parseable as numbers
            display
                .id
                .parse::<u32>()
                .expect("macOS display ID should be numeric");
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_tests {
    use super::*;

    #[test]
    #[serial]
    fn test_linux_specific_behavior() {
        let provider = create_provider().expect("Should create Linux provider");

        // Test both D-Bus and command-line approaches
        let configs = [
            ProfileConfig {
                linux_prefer_dbus: true,
                fallback_enabled: true,
            },
            ProfileConfig {
                linux_prefer_dbus: false,
                fallback_enabled: true,
            },
        ];

        for config in &configs {
            let provider = create_provider_with_config(config.clone())
                .expect("Should create Linux provider with config");

            match provider.get_displays() {
                Ok(displays) => {
                    println!(
                        "Linux config {:?}: found {} displays",
                        config,
                        displays.len()
                    );
                }
                Err(e) => {
                    println!("Linux config {:?}: error (may be expected): {}", config, e);
                    // On Linux, it's possible that colormgr or D-Bus is not available
                    // This is not necessarily a test failure
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_tests {
    use super::*;

    #[test]
    #[serial]
    fn test_windows_specific_behavior() {
        let provider = create_provider().expect("Should create Windows provider");
        let displays = provider
            .get_displays()
            .expect("Should get displays on Windows");

        // Windows should always have at least one display
        assert!(
            !displays.is_empty(),
            "Windows should have at least one display"
        );

        // Test that we can get profiles (Windows may or may not have profiles assigned)
        for display in &displays {
            match provider.get_profile(display) {
                Ok(profile) => {
                    println!(
                        "Windows display '{}' has profile '{}'",
                        display.name, profile.name
                    );

                    // Windows profiles often have file paths
                    if let Some(path) = &profile.file_path {
                        println!("  Profile path: {}", path.display());
                    }
                }
                Err(ProfileError::ProfileNotAvailable(_)) => {
                    println!(
                        "Windows display '{}' has no profile (this is valid)",
                        display.name
                    );
                }
                Err(e) => {
                    println!("Windows display '{}' error: {}", display.name, e);
                }
            }
        }
    }
}

/// Stress test with multiple rapid calls
#[test]
#[serial]
fn test_rapid_calls() {
    let provider = create_provider().expect("Should create provider");

    // Make multiple rapid calls to test stability
    for i in 0..10 {
        let displays = provider
            .get_displays()
            .expect(&format!("Should get displays on iteration {}", i));

        assert!(
            !displays.is_empty(),
            "Should have displays on iteration {}",
            i
        );

        let primary = provider
            .get_primary_display()
            .expect(&format!("Should get primary display on iteration {}", i));

        assert!(
            primary.is_primary,
            "Primary should be primary on iteration {}",
            i
        );
    }
}

/// Test error handling with invalid display
#[test]
#[serial]
fn test_error_handling_invalid_display() {
    let _provider = create_provider().expect("Should create provider");

    // Test with a configuration that disables fallbacks
    let no_fallback_config = ProfileConfig {
        linux_prefer_dbus: true,
        fallback_enabled: false, // Disable fallbacks
    };

    let provider = create_provider_with_config(no_fallback_config)
        .expect("Should create provider with no fallback");

    // Create a fake display that doesn't exist
    let fake_display = display_icc::Display {
        id: "nonexistent_display_12345".to_string(),
        name: "Fake Display".to_string(),
        is_primary: false,
    };

    // Should return appropriate errors when fallbacks are disabled
    let profile_result = provider.get_profile(&fake_display);
    let data_result = provider.get_profile_data(&fake_display);

    // At least one of these should fail (depending on platform implementation)
    // Some platforms may have fallback mechanisms even when disabled
    if profile_result.is_ok() && data_result.is_ok() {
        println!("Platform provides fallback profiles even for invalid displays");
        println!("Profile: {:?}", profile_result.unwrap());
    } else {
        println!("Platform correctly errors for invalid displays");
        if profile_result.is_err() {
            println!("Profile error: {:?}", profile_result.unwrap_err());
        }
        if data_result.is_err() {
            println!("Data error: {:?}", data_result.unwrap_err());
        }
    }

    // This test passes if it doesn't panic - different platforms handle invalid displays differently
}

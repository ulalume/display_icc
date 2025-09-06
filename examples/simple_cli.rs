//! Simple CLI example showing basic usage of display_icc library.
//!
//! This example demonstrates the most common use cases for the display_icc library:
//! - Getting the primary display profile
//! - Listing all display profiles
//! - Handling errors gracefully
//!
//! Run with: cargo run --example simple_cli

use display_icc::{
    get_primary_display_profile, get_all_display_profiles, 
    ProfileError, ColorSpace
};

fn main() {
    println!("display_icc Simple CLI Example");
    println!("==============================\n");

    // Example 1: Get primary display profile
    match get_primary_display_profile() {
        Ok(profile) => {
            println!("✓ Primary Display Profile:");
            println!("  Name: {}", profile.name);
            println!("  Color Space: {}", profile.color_space);
            
            if let Some(description) = &profile.description {
                println!("  Description: {}", description);
            }
            
            if let Some(path) = &profile.file_path {
                println!("  File Path: {}", path.display());
            }
            
            // Check for common color spaces
            match profile.color_space {
                ColorSpace::RGB => println!("  → This is an RGB color space (most common)"),
                ColorSpace::Lab => println!("  → This is a Lab color space (high precision)"),
                ColorSpace::Unknown => println!("  → Unknown color space"),
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to get primary display profile: {}", e);
            handle_error(&e);
        }
    }

    println!();

    // Example 2: List all display profiles
    match get_all_display_profiles() {
        Ok(profiles) => {
            println!("✓ All Display Profiles ({} found):", profiles.len());
            
            if profiles.is_empty() {
                println!("  No displays with profiles found.");
            } else {
                for (i, (display, profile)) in profiles.iter().enumerate() {
                    println!("  {}. Display: {} ({})", 
                             i + 1, 
                             display.name, 
                             if display.is_primary { "Primary" } else { "Secondary" });
                    println!("     Profile: {} ({})", profile.name, profile.color_space);
                    
                    if let Some(path) = &profile.file_path {
                        println!("     File: {}", path.display());
                    }
                    
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to get display profiles: {}", e);
            handle_error(&e);
        }
    }

    // Example 3: Platform detection
    match display_icc::detect_platform() {
        Ok(platform) => {
            println!("✓ Running on platform: {}", platform);
            
            // Show platform-specific notes
            match platform {
                display_icc::Platform::MacOS => {
                    println!("  → Using CoreGraphics framework");
                    println!("  → Profiles typically in /System/Library/ColorSync/Profiles/");
                }
                display_icc::Platform::Linux => {
                    println!("  → Using colormgr/colord system");
                    println!("  → Profiles typically in /usr/share/color/icc/");
                }
                display_icc::Platform::Windows => {
                    println!("  → Using Win32 Color System API");
                    println!("  → Profiles typically in C:\\WINDOWS\\System32\\spool\\drivers\\color\\");
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Platform detection failed: {}", e);
        }
    }
}

/// Handle different types of errors with helpful messages
fn handle_error(error: &ProfileError) {
    match error {
        ProfileError::UnsupportedPlatform => {
            eprintln!("  Help: This platform is not supported by display_icc.");
            eprintln!("        Supported platforms: macOS, Linux, Windows");
        }
        ProfileError::DisplayNotFound(id) => {
            eprintln!("  Help: Display '{}' was not found.", id);
            eprintln!("        Try running without specifying a display ID to use the primary display.");
        }
        ProfileError::ProfileNotAvailable(display) => {
            eprintln!("  Help: Display '{}' has no ICC profile assigned.", display);
            eprintln!("        This is normal for displays using default system profiles.");
        }
        ProfileError::SystemError(msg) => {
            eprintln!("  Help: System API error occurred: {}", msg);
            eprintln!("        This might be a temporary issue or permission problem.");
        }
        ProfileError::IoError(msg) => {
            eprintln!("  Help: File I/O error: {}", msg);
            eprintln!("        Check file permissions and disk space.");
        }
        ProfileError::ParseError(msg) => {
            eprintln!("  Help: Data parsing error: {}", msg);
            eprintln!("        The profile data might be corrupted or in an unsupported format.");
        }
    }
}
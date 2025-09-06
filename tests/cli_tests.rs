//! CLI integration tests for display_icc
//!
//! These tests verify the command-line interface works correctly
//! with various argument combinations.

use serial_test::serial;
use std::process::Command;
use tempfile::NamedTempFile;

/// Helper function to run the CLI with arguments
fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("display_icc")
        .arg("--")
        .args(args)
        .output()
        .expect("Failed to execute CLI")
}

/// Helper function to check if output is valid JSON
fn is_valid_json(output: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(output).is_ok()
}

#[test]
#[serial]
fn test_cli_info_command() {
    let output = run_cli(&["info"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI info output:\n{}", stdout);

        // Should contain display information
        assert!(
            stdout.contains("Display:"),
            "Should show display information"
        );
        assert!(stdout.contains("Primary:"), "Should show primary status");

        // May contain profile information if available
        if stdout.contains("Profile:") {
            assert!(
                stdout.contains("Color space:"),
                "Should show color space if profile exists"
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "CLI info failed (may be expected on some systems): {}",
            stderr
        );

        // On some systems (like CI), display access might not be available
        // This is not necessarily a test failure
    }
}

#[test]
#[serial]
fn test_cli_info_json_format() {
    let output = run_cli(&["info", "--format", "json"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI info JSON output:\n{}", stdout);

        // Should be valid JSON
        assert!(is_valid_json(&stdout), "Output should be valid JSON");

        // Parse and verify structure
        let json: serde_json::Value = serde_json::from_str(&stdout).expect("Should parse as JSON");

        assert!(json.get("display").is_some(), "Should have display object");

        let display = &json["display"];
        assert!(display.get("id").is_some(), "Display should have id");
        assert!(display.get("name").is_some(), "Display should have name");
        assert!(
            display.get("is_primary").is_some(),
            "Display should have is_primary"
        );

        // Profile may or may not be present
        if json.get("profile").is_some() {
            let profile = &json["profile"];
            assert!(profile.get("name").is_some(), "Profile should have name");
            assert!(
                profile.get("color_space").is_some(),
                "Profile should have color_space"
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI info JSON failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_info_verbose() {
    let output = run_cli(&["info", "--verbose"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI info verbose output:\n{}", stdout);

        // Verbose mode should show additional information
        assert!(
            stdout.contains("Display:"),
            "Should show display information"
        );

        // May show ICC information if profile is available
        if stdout.contains("ICC profile size:") {
            assert!(stdout.contains("bytes"), "Should show ICC size in bytes");
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI info verbose failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_list_command() {
    let output = run_cli(&["list"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI list output:\n{}", stdout);

        // Should show available displays
        assert!(
            stdout.contains("Available displays:") || stdout.contains("Display:"),
            "Should show display information"
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI list failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_list_json_format() {
    let output = run_cli(&["list", "--format", "json"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI list JSON output:\n{}", stdout);

        // Should be valid JSON
        assert!(is_valid_json(&stdout), "Output should be valid JSON");

        // Parse and verify structure
        let json: serde_json::Value = serde_json::from_str(&stdout).expect("Should parse as JSON");

        assert!(json.get("displays").is_some(), "Should have displays array");

        let displays = json["displays"]
            .as_array()
            .expect("displays should be an array");

        // Should have at least one display
        if !displays.is_empty() {
            let first_display = &displays[0];
            assert!(first_display.get("id").is_some(), "Display should have id");
            assert!(
                first_display.get("name").is_some(),
                "Display should have name"
            );
            assert!(
                first_display.get("is_primary").is_some(),
                "Display should have is_primary"
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI list JSON failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_list_verbose() {
    let output = run_cli(&["list", "--verbose"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI list verbose output:\n{}", stdout);

        // Verbose mode should show additional details
        assert!(
            stdout.contains("Display:") || stdout.contains("Available displays:"),
            "Should show display information"
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI list verbose failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_export_command() {
    let temp_file = NamedTempFile::new().expect("Should create temp file");
    let temp_path = temp_file.path().to_str().expect("Should get temp path");

    let output = run_cli(&["export", "--output", temp_path]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI export output:\n{}", stdout);

        // Should indicate successful export
        assert!(
            stdout.contains("Exported") || stdout.contains("Profile size:"),
            "Should show export confirmation"
        );

        // Check that file was created and has content
        let file_size = std::fs::metadata(temp_path)
            .expect("Exported file should exist")
            .len();

        assert!(file_size > 0, "Exported file should not be empty");
        assert!(file_size >= 128, "ICC profile should be at least 128 bytes");

        println!("Exported ICC profile size: {} bytes", file_size);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI export failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_export_json_format() {
    let temp_file = NamedTempFile::new().expect("Should create temp file");
    let temp_path = temp_file.path().to_str().expect("Should get temp path");

    let output = run_cli(&["export", "--output", temp_path, "--format", "json"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI export JSON output:\n{}", stdout);

        // Should be valid JSON
        assert!(is_valid_json(&stdout), "Output should be valid JSON");

        // Parse and verify structure
        let json: serde_json::Value = serde_json::from_str(&stdout).expect("Should parse as JSON");

        assert!(json.get("success").is_some(), "Should have success field");
        assert!(json.get("display").is_some(), "Should have display object");
        assert!(
            json.get("output_file").is_some(),
            "Should have output_file field"
        );
        assert!(
            json.get("size_bytes").is_some(),
            "Should have size_bytes field"
        );

        // Verify the file was actually created
        let output_file = json["output_file"]
            .as_str()
            .expect("output_file should be a string");
        assert_eq!(output_file, temp_path, "Output file path should match");

        let file_exists = std::fs::metadata(temp_path).is_ok();
        assert!(file_exists, "Exported file should exist");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI export JSON failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_header_command() {
    let output = run_cli(&["header"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI header output:\n{}", stdout);

        // Should show ICC header information
        assert!(
            stdout.contains("ICC Profile Header") || stdout.contains("Profile size:"),
            "Should show ICC header information"
        );

        if stdout.contains("Version:") {
            assert!(stdout.contains("Device class:"), "Should show device class");
            assert!(
                stdout.contains("Data color space:"),
                "Should show color space"
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI header failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_header_json_format() {
    let output = run_cli(&["header", "--format", "json"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI header JSON output:\n{}", stdout);

        // Should be valid JSON
        assert!(is_valid_json(&stdout), "Output should be valid JSON");

        // Parse and verify structure
        let json: serde_json::Value = serde_json::from_str(&stdout).expect("Should parse as JSON");

        assert!(json.get("display").is_some(), "Should have display object");
        assert!(
            json.get("icc_header").is_some(),
            "Should have icc_header object"
        );

        let header = &json["icc_header"];
        assert!(
            header.get("profile_size").is_some(),
            "Header should have profile_size"
        );
        assert!(
            header.get("version").is_some(),
            "Header should have version"
        );
        assert!(
            header.get("device_class").is_some(),
            "Header should have device_class"
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI header JSON failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_help() {
    let output = run_cli(&["--help"]);

    // Help should always work
    assert!(output.status.success(), "Help command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("CLI help output:\n{}", stdout);

    // Should contain usage information
    assert!(stdout.contains("display_icc"), "Should show program name");
    assert!(
        stdout.contains("USAGE") || stdout.contains("Usage"),
        "Should show usage"
    );
    assert!(
        stdout.contains("COMMANDS") || stdout.contains("Commands"),
        "Should show commands"
    );

    // Should list all main commands
    assert!(stdout.contains("info"), "Should list info command");
    assert!(stdout.contains("list"), "Should list list command");
    assert!(stdout.contains("export"), "Should list export command");
    assert!(stdout.contains("header"), "Should list header command");
}

#[test]
#[serial]
fn test_cli_version() {
    let output = run_cli(&["--version"]);

    // Version should always work
    assert!(output.status.success(), "Version command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("CLI version output:\n{}", stdout);

    // Should contain version information
    assert!(
        stdout.contains("display_icc") || stdout.contains("0.1.0"),
        "Should show version information"
    );
}

#[test]
#[serial]
fn test_cli_invalid_command() {
    let output = run_cli(&["invalid_command"]);

    // Invalid command should fail
    assert!(!output.status.success(), "Invalid command should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("CLI invalid command error:\n{}", stderr);

    // Should show error message
    assert!(
        !stderr.is_empty(),
        "Should show error message for invalid command"
    );
}

#[test]
#[serial]
fn test_cli_platform_specific_options() {
    // Test Linux-specific options
    #[cfg(target_os = "linux")]
    {
        let output = run_cli(&["info", "--prefer-command"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("CLI with --prefer-command:\n{}", stdout);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("CLI --prefer-command failed (may be expected): {}", stderr);
        }
    }

    // Test fallback options
    let output = run_cli(&["info", "--no-fallback"]);

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("CLI with --no-fallback:\n{}", stdout);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("CLI --no-fallback failed (may be expected): {}", stderr);
    }
}

#[test]
#[serial]
fn test_cli_output_consistency() {
    // Test that the same command produces consistent output
    let output1 = run_cli(&["info", "--format", "json"]);
    let output2 = run_cli(&["info", "--format", "json"]);

    if output1.status.success() && output2.status.success() {
        let stdout1 = String::from_utf8_lossy(&output1.stdout);
        let stdout2 = String::from_utf8_lossy(&output2.stdout);

        // Parse both outputs as JSON
        if let (Ok(json1), Ok(json2)) = (
            serde_json::from_str::<serde_json::Value>(&stdout1),
            serde_json::from_str::<serde_json::Value>(&stdout2),
        ) {
            // Display information should be consistent
            assert_eq!(
                json1.get("display"),
                json2.get("display"),
                "Display information should be consistent between calls"
            );

            // Profile information should also be consistent (if present)
            if json1.get("profile").is_some() && json2.get("profile").is_some() {
                assert_eq!(
                    json1.get("profile"),
                    json2.get("profile"),
                    "Profile information should be consistent between calls"
                );
            }
        }
    }
}

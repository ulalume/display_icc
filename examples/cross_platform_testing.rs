//! Cross-platform testing example for display_icc library.
//!
//! This example demonstrates how to test display_icc functionality across
//! different platforms and configurations, including:
//! - Platform-specific behavior testing
//! - Configuration option testing
//! - Error condition simulation
//! - Performance benchmarking
//! - Compatibility verification
//!
//! Run with: cargo run --example cross_platform_testing

use display_icc::{
    create_provider, create_provider_with_config, detect_platform, get_all_display_profiles,
    get_primary_display_profile, parse_icc_header, Platform, ProfileConfig, ProfileError,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Test results for different configurations
#[derive(Debug)]
struct TestResult {
    success: bool,
    duration: Duration,
    error: Option<String>,
    details: HashMap<String, String>,
}

impl TestResult {
    fn success(duration: Duration) -> Self {
        Self {
            success: true,
            duration,
            error: None,
            details: HashMap::new(),
        }
    }

    fn failure(duration: Duration, error: String) -> Self {
        Self {
            success: false,
            duration,
            error: Some(error),
            details: HashMap::new(),
        }
    }

    fn with_detail(mut self, key: &str, value: String) -> Self {
        self.details.insert(key.to_string(), value);
        self
    }
}

/// Cross-platform test suite
struct CrossPlatformTester {
    platform: Platform,
    results: HashMap<String, TestResult>,
}

impl CrossPlatformTester {
    fn new() -> Result<Self, ProfileError> {
        let platform = detect_platform()?;
        Ok(Self {
            platform,
            results: HashMap::new(),
        })
    }

    /// Run all tests
    fn run_all_tests(&mut self) {
        println!("🧪 Running Cross-Platform Tests");
        println!("===============================");
        println!("Platform: {}\n", self.platform);

        // Basic functionality tests
        self.test_basic_functionality();
        self.test_configuration_options();
        self.test_error_conditions();
        self.test_performance();
        self.test_platform_specific_features();

        // Print summary
        self.print_test_summary();
    }

    /// Test basic library functionality
    fn test_basic_functionality(&mut self) {
        println!("📋 Testing Basic Functionality");
        println!("------------------------------");

        // Test 1: Platform detection
        let start = Instant::now();
        match detect_platform() {
            Ok(platform) => {
                let result = TestResult::success(start.elapsed())
                    .with_detail("detected_platform", platform.to_string());
                self.results
                    .insert("platform_detection".to_string(), result);
                println!(
                    "✅ Platform detection: {} ({:?})",
                    platform,
                    start.elapsed()
                );
            }
            Err(e) => {
                let result = TestResult::failure(start.elapsed(), e.to_string());
                self.results
                    .insert("platform_detection".to_string(), result);
                println!("❌ Platform detection failed: {}", e);
            }
        }

        // Test 2: Provider creation
        let start = Instant::now();
        match create_provider() {
            Ok(_) => {
                let result = TestResult::success(start.elapsed());
                self.results.insert("provider_creation".to_string(), result);
                println!("✅ Provider creation: {:?}", start.elapsed());
            }
            Err(e) => {
                let result = TestResult::failure(start.elapsed(), e.to_string());
                self.results.insert("provider_creation".to_string(), result);
                println!("❌ Provider creation failed: {}", e);
                return; // Can't continue without provider
            }
        }

        // Test 3: Primary display profile
        let start = Instant::now();
        match get_primary_display_profile() {
            Ok(profile) => {
                let result = TestResult::success(start.elapsed())
                    .with_detail("profile_name", profile.name.clone())
                    .with_detail("color_space", profile.color_space.to_string())
                    .with_detail("has_file_path", profile.file_path.is_some().to_string());
                self.results.insert("primary_profile".to_string(), result);
                println!(
                    "✅ Primary profile: {} ({}) - {:?}",
                    profile.name,
                    profile.color_space,
                    start.elapsed()
                );
            }
            Err(e) => {
                let result = TestResult::failure(start.elapsed(), e.to_string());
                self.results.insert("primary_profile".to_string(), result);
                println!("❌ Primary profile failed: {}", e);
            }
        }

        // Test 4: All display profiles
        let start = Instant::now();
        match get_all_display_profiles() {
            Ok(profiles) => {
                let result = TestResult::success(start.elapsed())
                    .with_detail("display_count", profiles.len().to_string());
                self.results.insert("all_profiles".to_string(), result);
                println!(
                    "✅ All profiles: {} displays found - {:?}",
                    profiles.len(),
                    start.elapsed()
                );

                for (i, (display, profile)) in profiles.iter().enumerate() {
                    println!(
                        "   {}. {} -> {} ({})",
                        i + 1,
                        display.name,
                        profile.name,
                        profile.color_space
                    );
                }
            }
            Err(e) => {
                let result = TestResult::failure(start.elapsed(), e.to_string());
                self.results.insert("all_profiles".to_string(), result);
                println!("❌ All profiles failed: {}", e);
            }
        }

        println!();
    }

    /// Test different configuration options
    fn test_configuration_options(&mut self) {
        println!("⚙️  Testing Configuration Options");
        println!("--------------------------------");

        let configs = vec![
            ("default", ProfileConfig::default()),
            (
                "no_fallback",
                ProfileConfig {
                    linux_prefer_dbus: true,
                    fallback_enabled: false,
                },
            ),
        ];

        // Only test linux_prefer_dbus on Linux
        let mut linux_configs = configs.clone();
        if matches!(self.platform, Platform::Linux) {
            linux_configs.push((
                "prefer_command",
                ProfileConfig {
                    linux_prefer_dbus: false,
                    fallback_enabled: true,
                },
            ));
        }

        for (name, config) in linux_configs {
            let start = Instant::now();
            match create_provider_with_config(config) {
                Ok(provider) => match provider.get_primary_display() {
                    Ok(display) => match provider.get_profile(&display) {
                        Ok(profile) => {
                            let result = TestResult::success(start.elapsed())
                                .with_detail("profile_name", profile.name.clone());
                            self.results.insert(format!("config_{}", name), result);
                            println!(
                                "✅ Config '{}': {} - {:?}",
                                name,
                                profile.name,
                                start.elapsed()
                            );
                        }
                        Err(e) => {
                            let result = TestResult::failure(start.elapsed(), e.to_string());
                            self.results.insert(format!("config_{}", name), result);
                            println!("❌ Config '{}' profile failed: {}", name, e);
                        }
                    },
                    Err(e) => {
                        let result = TestResult::failure(start.elapsed(), e.to_string());
                        self.results.insert(format!("config_{}", name), result);
                        println!("❌ Config '{}' display failed: {}", name, e);
                    }
                },
                Err(e) => {
                    let result = TestResult::failure(start.elapsed(), e.to_string());
                    self.results.insert(format!("config_{}", name), result);
                    println!("❌ Config '{}' provider failed: {}", name, e);
                }
            }
        }

        println!();
    }

    /// Test error conditions and edge cases
    fn test_error_conditions(&mut self) {
        println!("🚨 Testing Error Conditions");
        println!("---------------------------");

        // Test 1: Invalid display ID
        let start = Instant::now();
        if let Ok(provider) = create_provider() {
            let fake_display = display_icc::Display {
                id: "nonexistent_display_12345".to_string(),
                name: "Fake Display".to_string(),
                is_primary: false,
            };

            match provider.get_profile(&fake_display) {
                Ok(_) => {
                    println!("⚠️  Unexpected success with fake display");
                }
                Err(ProfileError::DisplayNotFound(_)) => {
                    let result = TestResult::success(start.elapsed());
                    self.results
                        .insert("error_invalid_display".to_string(), result);
                    println!("✅ Invalid display error handling: {:?}", start.elapsed());
                }
                Err(e) => {
                    println!("⚠️  Unexpected error type for fake display: {}", e);
                }
            }
        }

        // Test 2: ICC header parsing with invalid data
        let start = Instant::now();
        let invalid_icc_data = vec![0u8; 64]; // Too short for ICC header
        match parse_icc_header(&invalid_icc_data) {
            Ok(_) => {
                println!("⚠️  Unexpected success parsing invalid ICC data");
            }
            Err(ProfileError::ParseError(_)) => {
                let result = TestResult::success(start.elapsed());
                self.results.insert("error_invalid_icc".to_string(), result);
                println!("✅ Invalid ICC data error handling: {:?}", start.elapsed());
            }
            Err(e) => {
                println!("⚠️  Unexpected error type for invalid ICC: {}", e);
            }
        }

        println!();
    }

    /// Test performance characteristics
    fn test_performance(&mut self) {
        println!("⚡ Testing Performance");
        println!("---------------------");

        // Test 1: Repeated profile access
        let iterations = 10;
        let mut durations = Vec::new();

        for i in 0..iterations {
            let start = Instant::now();
            match get_primary_display_profile() {
                Ok(_) => {
                    durations.push(start.elapsed());
                }
                Err(e) => {
                    println!("❌ Performance test iteration {} failed: {}", i + 1, e);
                    break;
                }
            }
        }

        if !durations.is_empty() {
            let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;
            let min_duration = durations.iter().min().unwrap();
            let max_duration = durations.iter().max().unwrap();

            let result = TestResult::success(avg_duration)
                .with_detail("min_duration", format!("{:?}", min_duration))
                .with_detail("max_duration", format!("{:?}", max_duration))
                .with_detail("iterations", iterations.to_string());
            self.results
                .insert("performance_repeated".to_string(), result);

            println!("✅ Repeated access ({} iterations):", iterations);
            println!("   Average: {:?}", avg_duration);
            println!("   Range: {:?} - {:?}", min_duration, max_duration);
        }

        // Performance testing completed
        println!("✅ Performance testing completed");

        println!();
    }

    /// Test platform-specific features
    fn test_platform_specific_features(&mut self) {
        println!("🖥️  Testing Platform-Specific Features");
        println!("--------------------------------------");

        match self.platform {
            Platform::MacOS => {
                println!("🍎 macOS-specific tests:");
                println!("   • CoreGraphics API integration");
                println!("   • Multiple display support");
                println!("   • Built-in display profile handling");

                // Test multiple displays if available
                if let Ok(provider) = create_provider() {
                    match provider.get_displays() {
                        Ok(displays) => {
                            let builtin_displays = displays
                                .iter()
                                .filter(|d| {
                                    d.name.contains("Built-in") || d.name.contains("Retina")
                                })
                                .count();
                            let external_displays = displays.len() - builtin_displays;

                            println!(
                                "   ✅ Found {} built-in, {} external displays",
                                builtin_displays, external_displays
                            );
                        }
                        Err(e) => println!("   ❌ Display enumeration failed: {}", e),
                    }
                }
            }
            Platform::Linux => {
                println!("🐧 Linux-specific tests:");
                println!("   • colormgr/colord integration");
                println!("   • D-Bus API vs command-line tool");
                println!("   • Profile file system access");

                // Test both D-Bus and command preferences
                let dbus_config = ProfileConfig {
                    linux_prefer_dbus: true,
                    fallback_enabled: false,
                };

                let command_config = ProfileConfig {
                    linux_prefer_dbus: false,
                    fallback_enabled: false,
                };

                let dbus_result =
                    create_provider_with_config(dbus_config).and_then(|p| p.get_displays());
                let command_result =
                    create_provider_with_config(command_config).and_then(|p| p.get_displays());

                match (dbus_result, command_result) {
                    (Ok(_), Ok(_)) => println!("   ✅ Both D-Bus and command methods work"),
                    (Ok(_), Err(_)) => println!("   ⚠️  D-Bus works, command method failed"),
                    (Err(_), Ok(_)) => println!("   ⚠️  Command works, D-Bus method failed"),
                    (Err(e1), Err(e2)) => {
                        println!("   ❌ Both methods failed: D-Bus({}), Command({})", e1, e2)
                    }
                }
            }
            Platform::Windows => {
                println!("🪟 Windows-specific tests:");
                println!("   • Win32 Color System API");
                println!("   • Registry-based profile lookup");
                println!("   • Windows color directory access");

                // Test profile file access
                if let Ok(provider) = create_provider() {
                    if let Ok(primary) = provider.get_primary_display() {
                        match provider.get_profile(&primary) {
                            Ok(profile) => {
                                if let Some(path) = &profile.file_path {
                                    if path.exists() {
                                        println!(
                                            "   ✅ Profile file accessible: {}",
                                            path.display()
                                        );
                                    } else {
                                        println!("   ⚠️  Profile file path exists but file not found: {}", path.display());
                                    }
                                } else {
                                    println!("   📊 Profile has no file path (embedded profile)");
                                }
                            }
                            Err(e) => println!("   ❌ Profile access failed: {}", e),
                        }
                    }
                }
            }
        }

        println!();
    }

    /// Print test summary
    fn print_test_summary(&self) {
        println!("📊 Test Summary");
        println!("===============");

        let total_tests = self.results.len();
        let passed_tests = self.results.values().filter(|r| r.success).count();
        let failed_tests = total_tests - passed_tests;

        println!("Total tests: {}", total_tests);
        println!("Passed: {} ✅", passed_tests);
        println!("Failed: {} ❌", failed_tests);

        if failed_tests > 0 {
            println!("\nFailed tests:");
            for (name, result) in &self.results {
                if !result.success {
                    println!(
                        "  ❌ {}: {}",
                        name,
                        result
                            .error
                            .as_ref()
                            .unwrap_or(&"Unknown error".to_string())
                    );
                }
            }
        }

        println!("\nPerformance summary:");
        for (name, result) in &self.results {
            if result.success && result.duration > Duration::from_millis(100) {
                println!("  ⏱️  {}: {:?}", name, result.duration);
            }
        }

        let success_rate = (passed_tests as f64 / total_tests as f64) * 100.0;
        println!("\nSuccess rate: {:.1}%", success_rate);

        if success_rate >= 90.0 {
            println!("🎉 Excellent compatibility!");
        } else if success_rate >= 75.0 {
            println!("👍 Good compatibility with some issues");
        } else {
            println!("⚠️  Significant compatibility issues detected");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("display_icc Cross-Platform Testing Example");
    println!("==========================================\n");

    let mut tester = CrossPlatformTester::new()?;
    tester.run_all_tests();

    println!("\n💡 Testing Tips:");
    println!("   • Run this example on different platforms to compare behavior");
    println!("   • Test with different display configurations (single/multi-monitor)");
    println!("   • Try with and without color management software installed");
    println!("   • Monitor performance with different configuration options");
    println!("   • Test error handling by disconnecting displays during execution");

    Ok(())
}

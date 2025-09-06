//! GUI integration example for display_icc library.
//!
//! This example demonstrates how to integrate display_icc into a GUI application,
//! showing patterns for:
//! - Asynchronous profile loading
//! - Caching and performance optimization
//! - Multi-display handling
//! - Error handling in GUI contexts
//! - Profile change monitoring simulation
//!
//! This example uses a simulated GUI framework but shows real patterns
//! that would apply to actual GUI frameworks like egui, iced, tauri, etc.
//!
//! Run with: cargo run --example gui_integration

use display_icc::{
    create_provider_with_config, Display, DisplayProfileProvider, ProfileConfig, ProfileError,
    ProfileInfo,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Simulated GUI application state
#[derive(Debug)]
struct AppState {
    displays: Vec<Display>,
    profiles: HashMap<String, ProfileInfo>,
    last_update: Instant,
    error_message: Option<String>,
    is_loading: bool,
}

impl AppState {
    fn new() -> Self {
        Self {
            displays: Vec::new(),
            profiles: HashMap::new(),
            last_update: Instant::now(),
            error_message: None,
            is_loading: false,
        }
    }
}

/// Profile manager for GUI applications
struct ProfileManager {
    provider: Box<dyn DisplayProfileProvider>,
    state: Arc<Mutex<AppState>>,
}

impl ProfileManager {
    /// Create a new profile manager with optimized configuration for GUI use
    fn new() -> Result<Self, ProfileError> {
        // Configuration optimized for GUI applications
        let config = ProfileConfig {
            linux_prefer_dbus: true, // Use faster D-Bus API on Linux
            fallback_enabled: true,  // Ensure reliability
        };

        let provider = create_provider_with_config(config)?;
        let state = Arc::new(Mutex::new(AppState::new()));

        Ok(Self { provider, state })
    }

    /// Load display profiles asynchronously (simulated)
    fn load_profiles_async(&self) -> Result<(), ProfileError> {
        // Set loading state
        {
            let mut state = self.state.lock().unwrap();
            state.is_loading = true;
            state.error_message = None;
        }

        // Simulate async loading (in real GUI app, this would be in a separate thread)
        let displays = self.provider.get_displays()?;
        let mut profiles = HashMap::new();

        for display in &displays {
            match self.provider.get_profile(display) {
                Ok(profile) => {
                    profiles.insert(display.id.clone(), profile);
                }
                Err(ProfileError::ProfileNotAvailable(_)) => {
                    // Skip displays without profiles - this is normal
                    continue;
                }
                Err(e) => {
                    // Log error but continue with other displays
                    eprintln!(
                        "Warning: Failed to get profile for display '{}': {}",
                        display.name, e
                    );
                }
            }
        }

        // Update state
        {
            let mut state = self.state.lock().unwrap();
            state.displays = displays;
            state.profiles = profiles;
            state.last_update = Instant::now();
            state.is_loading = false;
        }

        Ok(())
    }

    /// Get current application state (thread-safe)
    fn get_state(&self) -> AppState {
        let state = self.state.lock().unwrap();
        AppState {
            displays: state.displays.clone(),
            profiles: state.profiles.clone(),
            last_update: state.last_update,
            error_message: state.error_message.clone(),
            is_loading: state.is_loading,
        }
    }

    /// Check if profiles need refreshing (e.g., every 30 seconds)
    fn needs_refresh(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.last_update.elapsed() > Duration::from_secs(30)
    }

    /// Get profile for a specific display
    fn get_display_profile(&self, display_id: &str) -> Option<ProfileInfo> {
        let state = self.state.lock().unwrap();
        state.profiles.get(display_id).cloned()
    }

    /// Export profile to file (useful for GUI save dialogs)
    fn export_profile(&self, display_id: &str, file_path: &str) -> Result<(), ProfileError> {
        let display = {
            let state = self.state.lock().unwrap();
            state.displays.iter().find(|d| d.id == display_id).cloned()
        };

        if let Some(display) = display {
            let profile_data = self.provider.get_profile_data(&display)?;
            std::fs::write(file_path, profile_data)?;
            Ok(())
        } else {
            Err(ProfileError::DisplayNotFound(display_id.to_string()))
        }
    }
}

/// Simulated GUI framework functions
mod gui_framework {
    use super::*;

    pub fn render_display_list(state: &AppState) {
        println!("=== Display List Widget ===");

        if state.is_loading {
            println!("🔄 Loading display profiles...");
            return;
        }

        if let Some(error) = &state.error_message {
            println!("❌ Error: {}", error);
            return;
        }

        if state.displays.is_empty() {
            println!("No displays found.");
            return;
        }

        for display in &state.displays {
            let primary_indicator = if display.is_primary { " (Primary)" } else { "" };
            println!("🖥️  {} ({}){}", display.name, display.id, primary_indicator);

            if let Some(profile) = state.profiles.get(&display.id) {
                println!("   📊 Profile: {} ({})", profile.name, profile.color_space);

                if let Some(path) = &profile.file_path {
                    println!("   📁 File: {}", path.display());
                }
            } else {
                println!("   ⚪ No profile assigned");
            }
            println!();
        }

        println!("Last updated: {:?} ago", state.last_update.elapsed());
    }

    pub fn render_profile_details(profile: &ProfileInfo, display_name: &str) {
        println!("=== Profile Details Widget ===");
        println!("Display: {}", display_name);
        println!("Profile Name: {}", profile.name);
        println!("Color Space: {}", profile.color_space);

        if let Some(description) = &profile.description {
            println!("Description: {}", description);
        }

        if let Some(path) = &profile.file_path {
            println!("File Path: {}", path.display());
        }

        // Simulate color space specific UI elements
        match profile.color_space {
            display_icc::ColorSpace::RGB => {
                println!("🎨 RGB Color Space Features:");
                println!("   • Standard color gamut support");
                println!("   • Compatible with most applications");
            }
            display_icc::ColorSpace::Lab => {
                println!("🔬 Lab Color Space Features:");
                println!("   • High precision color representation");
                println!("   • Device-independent colors");
            }
            display_icc::ColorSpace::Unknown => {
                println!("❓ Unknown Color Space");
                println!("   • Profile may use specialized color space");
            }
        }
    }

    pub fn simulate_user_interaction() {
        println!("=== Simulated User Interactions ===");
        println!("👆 User clicked 'Refresh Profiles' button");
        println!("👆 User selected display for detailed view");
        println!("👆 User clicked 'Export Profile' button");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("display_icc GUI Integration Example");
    println!("===================================\n");

    // Initialize profile manager
    let manager = ProfileManager::new()?;

    // Simulate GUI application lifecycle
    println!("🚀 Starting GUI application...\n");

    // Initial profile loading
    println!("📥 Loading initial display profiles...");
    match manager.load_profiles_async() {
        Ok(()) => println!("✅ Profiles loaded successfully\n"),
        Err(e) => {
            eprintln!("❌ Failed to load profiles: {}\n", e);
            return Err(e.into());
        }
    }

    // Simulate GUI rendering loop
    for frame in 1..=5 {
        println!("--- GUI Frame {} ---", frame);

        let state = manager.get_state();

        // Render main display list
        gui_framework::render_display_list(&state);

        // Simulate user selecting a display for details
        if let Some(primary_display) = state.displays.iter().find(|d| d.is_primary) {
            if let Some(profile) = manager.get_display_profile(&primary_display.id) {
                gui_framework::render_profile_details(&profile, &primary_display.name);
            }
        }

        // Simulate user interactions
        if frame == 3 {
            gui_framework::simulate_user_interaction();

            // Simulate profile export
            if let Some(primary_display) = state.displays.iter().find(|d| d.is_primary) {
                let export_path = format!("exported_profile_frame_{}.icc", frame);
                match manager.export_profile(&primary_display.id, &export_path) {
                    Ok(()) => println!("💾 Profile exported to {}", export_path),
                    Err(e) => eprintln!("❌ Export failed: {}", e),
                }
            }
        }

        // Check if refresh is needed
        if manager.needs_refresh() {
            println!("🔄 Profiles need refresh (30+ seconds old)");
        }

        // Simulate frame delay
        thread::sleep(Duration::from_millis(100));
        println!();
    }

    // Simulate profile monitoring (would be in background thread in real app)
    println!("🔍 Simulating profile change monitoring...");
    println!("   In a real GUI app, you would:");
    println!("   • Monitor for display connection/disconnection events");
    println!("   • Watch for profile changes in the system");
    println!("   • Update the UI automatically when changes occur");
    println!("   • Handle errors gracefully with user notifications");

    println!("\n✨ GUI integration example completed!");

    Ok(())
}

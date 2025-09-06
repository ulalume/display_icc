//! Command-line interface for display_icc.
//!
//! This module provides a comprehensive CLI for the display_icc library,
//! allowing users to inspect, list, and export display ICC profiles from
//! the command line across macOS, Linux, and Windows platforms.
//!
//! # Features
//!
//! - **Profile inspection**: View detailed information about display profiles
//! - **Multi-display support**: List and work with all connected displays
//! - **Profile export**: Save ICC profiles to files for backup or analysis
//! - **ICC header analysis**: Examine detailed ICC profile metadata
//! - **Multiple output formats**: Human-readable text and machine-readable JSON
//! - **Platform-specific options**: Configure behavior for different operating systems
//!
//! # Usage Examples
//!
//! ```bash
//! # Show primary display profile information
//! display_icc info
//!
//! # List all displays and their profiles
//! display_icc list
//!
//! # Export primary display profile to file
//! display_icc export --output my_profile.icc
//!
//! # Show detailed ICC header information
//! display_icc header
//!
//! # Get output in JSON format
//! display_icc info --format json
//!
//! # Verbose output with additional details
//! display_icc list --verbose
//!
//! # Work with specific display (use ID from list command)
//! display_icc info --display "69733382"
//! display_icc export --display "69733382" --output external_display.icc
//!
//! # Platform-specific options (Linux)
//! display_icc info --prefer-command --no-fallback
//! ```

use clap::{Parser, Subcommand, ValueEnum};
use display_icc::{parse_icc_header, ProfileConfig, ProfileError};
use std::fs;

/// Cross-platform tool for retrieving display ICC profiles
#[derive(Parser)]
#[command(name = "display_icc")]
#[command(about = "A cross-platform tool for retrieving display ICC color profiles")]
#[command(version = "0.1.0")]
#[command(long_about = "
display_icc is a cross-platform command-line tool for retrieving ICC color profiles 
from displays on macOS, Linux, and Windows. It provides both human-readable output 
and machine-readable JSON format for integration with other tools.

Examples:
  display_icc info                    # Show primary display profile info
  display_icc list                    # List all display profiles  
  display_icc export --output prof.icc  # Export primary display profile
  display_icc info --json             # Output in JSON format
  display_icc list --verbose          # Show detailed profile information
")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(short, long, value_enum, global = true)]
    format: Option<OutputFormat>,

    /// Enable verbose output with detailed information
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable fallback mechanisms (Linux/Windows only)
    #[arg(long, global = true)]
    no_fallback: bool,

    /// Prefer command-line tools over D-Bus API (Linux only)
    #[arg(long, global = true)]
    prefer_command: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show information about display profiles
    Info {
        /// Display ID to query (defaults to primary display)
        #[arg(short, long)]
        display: Option<String>,
    },
    /// List all available displays and their profiles
    List,
    /// Export ICC profile data to a file
    Export {
        /// Output file path for the ICC profile
        #[arg(short, long)]
        output: String,

        /// Display ID to export (defaults to primary display)
        #[arg(short, long)]
        display: Option<String>,
    },
    /// Show detailed ICC header information
    Header {
        /// Display ID to analyze (defaults to primary display)
        #[arg(short, long)]
        display: Option<String>,
    },
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output for programmatic use
    Json,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Create configuration based on CLI arguments
    let config = ProfileConfig {
        linux_prefer_dbus: !cli.prefer_command,
        fallback_enabled: !cli.no_fallback,
    };

    match &cli.command {
        Commands::Info { display } => {
            handle_info_command(display.clone(), &cli, config)?;
        }
        Commands::List => {
            handle_list_command(&cli, config)?;
        }
        Commands::Export { output, display } => {
            handle_export_command(output.clone(), display.clone(), &cli, config)?;
        }
        Commands::Header { display } => {
            handle_header_command(display.clone(), &cli, config)?;
        }
    }

    Ok(())
}

fn handle_info_command(
    display_id: Option<String>,
    cli: &Cli,
    config: ProfileConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let provider = display_icc::create_provider_with_config(config)?;

    let (display, profile) = if let Some(id) = display_id {
        // Find specific display
        let displays = provider.get_displays()?;
        let display = displays
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(ProfileError::DisplayNotFound(id))?;
        let profile = provider.get_profile(&display)?;
        (display, profile)
    } else {
        // Use primary display
        let display = provider.get_primary_display()?;
        let profile = provider.get_profile(&display)?;
        (display, profile)
    };

    match cli.format.as_ref().unwrap_or(&OutputFormat::Text) {
        OutputFormat::Text => {
            println!("Display: {} ({})", display.name, display.id);
            println!("Primary: {}", display.is_primary);
            println!("Profile: {}", profile.name);

            if let Some(desc) = &profile.description {
                println!("Description: {}", desc);
            }

            if let Some(path) = &profile.file_path {
                println!("File path: {}", path.display());
            }

            println!("Color space: {}", profile.color_space);

            if cli.verbose {
                // Show additional ICC data information
                match provider.get_profile_data(&display) {
                    Ok(icc_data) => {
                        println!("ICC profile size: {} bytes", icc_data.len());

                        if let Ok(header) = parse_icc_header(&icc_data) {
                            println!("ICC version: {}.{}", header.version.0, header.version.1);
                            println!("Device class: {}", header.device_class);
                            println!("Data color space: {}", header.data_color_space);
                            println!("Connection space: {}", header.connection_space);

                            if let Some(datetime) = &header.creation_datetime {
                                println!("Created: {}", datetime);
                            }

                            if !header.device_manufacturer.is_empty() {
                                println!("Manufacturer: {}", header.device_manufacturer);
                            }

                            if !header.device_model.is_empty() {
                                println!("Model: {}", header.device_model);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not retrieve ICC data: {}", e);
                    }
                }
            }
        }
        OutputFormat::Json => {
            let mut json_output = serde_json::json!({
                "display": {
                    "id": display.id,
                    "name": display.name,
                    "is_primary": display.is_primary
                },
                "profile": {
                    "name": profile.name,
                    "description": profile.description,
                    "file_path": profile.file_path.as_ref().map(|p| p.to_string_lossy()),
                    "color_space": profile.color_space.to_string()
                }
            });

            if cli.verbose {
                if let Ok(icc_data) = provider.get_profile_data(&display) {
                    json_output["icc_size"] = serde_json::Value::Number(icc_data.len().into());

                    if let Ok(header) = parse_icc_header(&icc_data) {
                        json_output["icc_header"] = serde_json::json!({
                            "version": format!("{}.{}", header.version.0, header.version.1),
                            "device_class": header.device_class,
                            "data_color_space": header.data_color_space,
                            "connection_space": header.connection_space,
                            "creation_datetime": header.creation_datetime,
                            "platform": header.platform,
                            "device_manufacturer": header.device_manufacturer,
                            "device_model": header.device_model
                        });
                    }
                }
            }

            println!("{}", serde_json::to_string_pretty(&json_output)?);
        }
    }

    Ok(())
}

fn handle_list_command(cli: &Cli, config: ProfileConfig) -> Result<(), Box<dyn std::error::Error>> {
    let provider = display_icc::create_provider_with_config(config)?;
    let displays = provider.get_displays()?;

    match cli.format.as_ref().unwrap_or(&OutputFormat::Text) {
        OutputFormat::Text => {
            println!("Available displays:");

            for display in displays {
                println!("\nDisplay: {} ({})", display.name, display.id);
                println!("  Primary: {}", display.is_primary);

                match provider.get_profile(&display) {
                    Ok(profile) => {
                        println!("  Profile: {}", profile.name);

                        if cli.verbose {
                            if let Some(desc) = &profile.description {
                                println!("  Description: {}", desc);
                            }
                            if let Some(path) = &profile.file_path {
                                println!("  File path: {}", path.display());
                            }
                            println!("  Color space: {}", profile.color_space);

                            if let Ok(icc_data) = provider.get_profile_data(&display) {
                                println!("  ICC size: {} bytes", icc_data.len());
                            }
                        }
                    }
                    Err(ProfileError::ProfileNotAvailable(_)) => {
                        println!("  Profile: No profile assigned");
                    }
                    Err(e) => {
                        println!("  Profile: Error - {}", e);
                    }
                }
            }
        }
        OutputFormat::Json => {
            let mut json_displays = Vec::new();

            for display in displays {
                let mut display_json = serde_json::json!({
                    "id": display.id,
                    "name": display.name,
                    "is_primary": display.is_primary
                });

                match provider.get_profile(&display) {
                    Ok(profile) => {
                        display_json["profile"] = serde_json::json!({
                            "name": profile.name,
                            "description": profile.description,
                            "file_path": profile.file_path.as_ref().map(|p| p.to_string_lossy()),
                            "color_space": profile.color_space.to_string()
                        });

                        if cli.verbose {
                            if let Ok(icc_data) = provider.get_profile_data(&display) {
                                display_json["icc_size"] =
                                    serde_json::Value::Number(icc_data.len().into());
                            }
                        }
                    }
                    Err(ProfileError::ProfileNotAvailable(_)) => {
                        display_json["profile"] = serde_json::Value::Null;
                    }
                    Err(e) => {
                        display_json["profile_error"] = serde_json::Value::String(e.to_string());
                    }
                }

                json_displays.push(display_json);
            }

            let output = serde_json::json!({
                "displays": json_displays
            });

            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

fn handle_export_command(
    output_path: String,
    display_id: Option<String>,
    cli: &Cli,
    config: ProfileConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let provider = display_icc::create_provider_with_config(config)?;

    let display = if let Some(id) = display_id {
        let displays = provider.get_displays()?;
        displays
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(ProfileError::DisplayNotFound(id))?
    } else {
        provider.get_primary_display()?
    };

    let icc_data = provider.get_profile_data(&display)?;
    fs::write(&output_path, &icc_data)?;

    match cli.format.as_ref().unwrap_or(&OutputFormat::Text) {
        OutputFormat::Text => {
            println!(
                "Exported ICC profile for display '{}' to '{}'",
                display.name, output_path
            );
            println!("Profile size: {} bytes", icc_data.len());
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "success": true,
                "display": {
                    "id": display.id,
                    "name": display.name
                },
                "output_file": output_path,
                "size_bytes": icc_data.len()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

fn handle_header_command(
    display_id: Option<String>,
    cli: &Cli,
    config: ProfileConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let provider = display_icc::create_provider_with_config(config)?;

    let display = if let Some(id) = display_id {
        let displays = provider.get_displays()?;
        displays
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(ProfileError::DisplayNotFound(id))?
    } else {
        provider.get_primary_display()?
    };

    let icc_data = provider.get_profile_data(&display)?;
    let header = parse_icc_header(&icc_data)?;

    match cli.format.as_ref().unwrap_or(&OutputFormat::Text) {
        OutputFormat::Text => {
            println!(
                "ICC Profile Header for display: {} ({})",
                display.name, display.id
            );
            println!("Profile size: {} bytes", header.profile_size);
            println!("Version: {}.{}", header.version.0, header.version.1);
            println!("Device class: {}", header.device_class);
            println!("Data color space: {}", header.data_color_space);
            println!("Connection space: {}", header.connection_space);

            if let Some(datetime) = &header.creation_datetime {
                println!("Created: {}", datetime);
            }

            println!("Platform: {}", header.platform);
            println!("Flags: 0x{:08X}", header.flags);

            if !header.preferred_cmm.is_empty() {
                println!("Preferred CMM: {}", header.preferred_cmm);
            }

            if !header.device_manufacturer.is_empty() {
                println!("Device manufacturer: {}", header.device_manufacturer);
            }

            if !header.device_model.is_empty() {
                println!("Device model: {}", header.device_model);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "display": {
                    "id": display.id,
                    "name": display.name
                },
                "icc_header": {
                    "profile_size": header.profile_size,
                    "preferred_cmm": header.preferred_cmm,
                    "version": format!("{}.{}", header.version.0, header.version.1),
                    "device_class": header.device_class,
                    "data_color_space": header.data_color_space,
                    "connection_space": header.connection_space,
                    "creation_datetime": header.creation_datetime,
                    "platform": header.platform,
                    "flags": format!("0x{:08X}", header.flags),
                    "device_manufacturer": header.device_manufacturer,
                    "device_model": header.device_model
                }
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}

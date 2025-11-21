pub mod analyzer;
pub mod entry_point;
pub mod framework;
pub mod rules;
pub mod test_utils;
pub mod utils;
pub mod visitor;

use crate::analyzer::Skylos;
use anyhow::Result;
use clap::Parser;
use colored::*;
use std::path::PathBuf;

/// Command line interface configuration using `clap`.
/// This struct defines the arguments and flags accepted by the program.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Python project to analyze.
    /// This is the root directory where the scan will begin.
    /// We need this to know which files to read and parse.
    path: PathBuf,

    /// Confidence threshold (0-100).
    /// Only findings with confidence higher than this value will be reported.
    /// This helps in filtering out false positives.
    #[arg(short, long, default_value_t = 60)]
    confidence: u8,

    /// Scan for API keys/secrets.
    /// If true, the analyzer will look for hardcoded secrets like API keys and passwords.
    #[arg(long)]
    secrets: bool,

    /// Scan for dangerous code.
    /// If true, the analyzer will look for dangerous patterns like eval(), exec(), etc.
    #[arg(long)]
    danger: bool,

    /// Scan for code quality issues.
    /// If true, the analyzer will check for code quality problems such as complex functions.
    #[arg(long)]
    quality: bool,

    /// Output raw JSON.
    /// If true, the output will be in JSON format for machine parsing.
    /// This is useful for integrating with other tools or CI/CD pipelines.
    #[arg(long)]
    json: bool,
}

/// Main entry point of the application.
///
/// This function handles argument parsing, initialization of the analyzer,
/// execution of the analysis, and output formatting.
fn main() -> Result<()> {
    // Parse command line arguments using the Cli struct definition.
    // This allows users to configure the analysis via CLI flags.
    let cli = Cli::parse();

    // If JSON output is not requested, print a friendly message indicating the start of analysis.
    // This gives immediate feedback to the user that the process is running.
    if !cli.json {
        println!("Analyzing path: {:?}", cli.path);
    }

    // Initialize the Skylos analyzer with the configuration from CLI.
    // We pass the confidence threshold and boolean flags for different types of checks.
    // This sets up the analyzer state before running on files.
    let skylos = Skylos::new(cli.confidence, cli.secrets, cli.danger, cli.quality);

    // Run the analysis on the provided path.
    // This traverses the directory, parses Python files, and applies rules.
    // It returns a Result containing the AnalysisResult struct or an error.
    // We propagate any error with `?`.
    let result = skylos.analyze(&cli.path)?;

    // Check if JSON output was requested.
    if cli.json {
        // Serialize the result struct to a pretty-printed JSON string.
        // This uses `serde_json` to convert the Rust struct to JSON.
        // This is useful for integrating with other tools or pipelines.
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        // If not JSON, print a human-readable report.

        // Print the header with bold text for visibility.
        println!("\n{}", "Python Static Analysis Results".bold());
        println!("===================================\n");

        // Print a summary of findings.
        // We check each category and print the count if it's not empty.
        println!("Summary:");
        if !result.unused_functions.is_empty() {
            println!(
                " * Unreachable functions: {}",
                result.unused_functions.len()
            );
        }
        if !result.unused_imports.is_empty() {
            println!(" * Unused imports: {}", result.unused_imports.len());
        }
        if !result.unused_classes.is_empty() {
            println!(" * Unused classes: {}", result.unused_classes.len());
        }
        if !result.unused_variables.is_empty() {
            println!(" * Unused variables: {}", result.unused_variables.len());
        }
        if cli.danger {
            println!(" * Security issues: {}", result.danger.len());
        }
        if cli.secrets {
            println!(" * Secrets found: {}", result.secrets.len());
        }
        if cli.quality {
            println!(" * Quality issues: {}", result.quality.len());
        }

        // List unused functions if any found.
        // We iterate over the results and print details like name, file path, and line number.
        if !result.unused_functions.is_empty() {
            println!("\n - Unreachable Functions");
            println!("=======================");
            for (i, func) in result.unused_functions.iter().enumerate() {
                println!(" {}. {}", i + 1, func.name);
                println!("    └─ {}:{}", func.file.display(), func.line);
            }
        }

        // List unused imports if any found.
        // Similarly, print details for unused imports.
        if !result.unused_imports.is_empty() {
            println!("\n - Unused Imports");
            println!("================");
            for (i, imp) in result.unused_imports.iter().enumerate() {
                println!(" {}. {}", i + 1, imp.simple_name);
                println!("    └─ {}:{}", imp.file.display(), imp.line);
            }
        }

        // List security issues if enabled and found.
        // We show the message, rule ID, location, and severity.
        if cli.danger && !result.danger.is_empty() {
            println!("\n - Security Issues");
            println!("================");
            for (i, f) in result.danger.iter().enumerate() {
                println!(
                    " {}. {} [{}] ({}:{}) Severity: {}",
                    i + 1,
                    f.message,
                    f.rule_id,
                    f.file.display(),
                    f.line,
                    f.severity
                );
            }
        }

        // List secrets if enabled and found.
        // We show the message, rule ID, location, and severity.
        if cli.secrets && !result.secrets.is_empty() {
            println!("\n - Secrets");
            println!("==========");
            for (i, s) in result.secrets.iter().enumerate() {
                println!(
                    " {}. {} [{}] ({}:{}) Severity: {}",
                    i + 1,
                    s.message,
                    s.rule_id,
                    s.file.display(),
                    s.line,
                    s.severity
                );
            }
        }

        // List quality issues if enabled and found.
        // We show the message, rule ID, location, and severity.
        if cli.quality && !result.quality.is_empty() {
            println!("\n - Quality Issues");
            println!("================");
            for (i, q) in result.quality.iter().enumerate() {
                println!(
                    " {}. {} [{}] ({}:{}) Severity: {}",
                    i + 1,
                    q.message,
                    q.rule_id,
                    q.file.display(),
                    q.line,
                    q.severity
                );
            }
        }
    }

    // Return Ok(()) to indicate successful execution.
    Ok(())
}

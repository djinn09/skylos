// Lib file to expose modules for testing and external usage.
// This file serves as the root for the library crate.

/// Module containing the core analyzer logic.
/// This includes the `Skylos` struct and its methods for running the analysis.
pub mod analyzer;

/// Module containing the AST visitor implementation.
/// This is responsible for traversing the Python AST and collecting data.
pub mod visitor;

/// Module defining the analysis result data structures.
/// This includes structs like `AnalysisResult`, `Finding`, `UnusedFunction`, etc.
pub mod framework;

/// Module containing test utilities.
/// This helps in writing tests for the analyzer and rules.
pub mod test_utils;

/// Module containing the implementation of various analysis rules.
/// This includes rules for security, quality, and secrets.
pub mod rules;

/// Module containing utility functions.
/// This includes helper functions used across the application.
pub mod utils;

/// Module defining the entry point logic.
/// This handles the integration with Python's setuptools/entry_points ecosystem if needed.
pub mod entry_point;

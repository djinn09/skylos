// Rules module
// This module exports the different categories of analysis rules.

/// Rules for detecting hardcoded secrets and credentials.
pub mod secrets;

/// Rules for detecting dangerous code patterns (security vulnerabilities).
pub mod danger;

/// Rules for detecting code quality issues.
pub mod quality;

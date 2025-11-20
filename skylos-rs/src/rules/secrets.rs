use regex::Regex;
use serde::Serialize;
use std::path::PathBuf;

/// Represents a secret finding (e.g., a hardcoded API key).
#[derive(Debug, Clone, Serialize)]
pub struct SecretFinding {
    /// Description of the finding.
    pub message: String,
    /// Unique rule identifier (e.g., "SKY-S101").
    pub rule_id: String,
    /// File where the secret was found.
    pub file: PathBuf,
    /// Line number.
    pub line: usize,
    /// Severity level (e.g., "HIGH").
    pub severity: String,
}

lazy_static::lazy_static! {
    /// Regular expressions for detecting secrets.
    /// Each entry is a tuple of (Description, Regex).
    static ref SECRET_PATTERNS: Vec<(&'static str, Regex)> = vec![
        // AWS Access Key ID: 20-char alphanumeric string starting with 'AKIA' usually (but we check 20 chars).
        // Pattern looks for assignment: aws_access_key_id = "..."
        ("AWS Access Key", Regex::new(r#"(?i)aws_access_key_id\s*=\s*['"][A-Z0-9]{20}['"]"#).unwrap()),

        // AWS Secret Access Key: 40-char base64-like string.
        // Pattern looks for assignment: aws_secret_access_key = "..."
        ("AWS Secret Key", Regex::new(r#"(?i)aws_secret_access_key\s*=\s*['"][A-Za-z0-9/+=]{40}['"]"#).unwrap()),

        // Generic API Key: Variables named api_key, secret, token with long string values.
        ("Generic API Key", Regex::new(r#"(?i)(api_key|apikey|secret|token)\s*=\s*['"][A-Za-z0-9_\-]{20,}['"]"#).unwrap()),
    ];
}

/// Scans the content of a file for secrets using regular expressions.
///
/// This function iterates through the file line by line and applies the regex patterns.
pub fn scan_secrets(content: &str, file_path: &PathBuf) -> Vec<SecretFinding> {
    let mut findings = Vec::new();
    
    for (line_idx, line) in content.lines().enumerate() {
        // Skip full-line comments to reduce false positives.
        // TODO: Improve comment detection (e.g., inline comments).
        if line.trim().starts_with('#') {
            continue;
        }

        // Check each pattern against the current line.
        for (name, regex) in SECRET_PATTERNS.iter() {
            if regex.is_match(line) {
                findings.push(SecretFinding {
                    message: format!("Found potential {}", name),
                    rule_id: "SKY-S101".to_string(),
                    file: file_path.clone(),
                    line: line_idx + 1,
                    severity: "HIGH".to_string(),
                });
            }
        }
    }
    
    findings
}

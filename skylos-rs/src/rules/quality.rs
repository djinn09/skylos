use crate::utils::LineIndex;
use rustpython_ast::{self as ast, ExceptHandler, Stmt};
use serde::Serialize;
use std::path::PathBuf;

/// Represents a code quality finding.
#[derive(Debug, Clone, Serialize)]
pub struct QualityFinding {
    /// Description of the issue.
    pub message: String,
    /// Unique rule identifier (e.g., "SKY-Q001").
    pub rule_id: String,
    /// File where the issue was found.
    pub file: PathBuf,
    /// Line number.
    pub line: usize,
    /// Severity level (e.g., "LOW").
    pub severity: String,
}

/// Visitor that checks for code quality issues.
///
/// Currently, it checks for deeply nested code blocks (cyclomatic complexity indicator).
pub struct QualityVisitor<'a> {
    /// Collected findings.
    pub findings: Vec<QualityFinding>,
    /// Current file path.
    pub file_path: PathBuf,
    /// Helper for line mapping.
    pub line_index: &'a LineIndex,
    /// Current nesting depth.
    pub current_depth: usize,
    /// Maximum allowed nesting depth before reporting an issue.
    pub max_depth: usize,
}

impl<'a> QualityVisitor<'a> {
    /// Creates a new `QualityVisitor`.
    pub fn new(file_path: PathBuf, line_index: &'a LineIndex) -> Self {
        Self {
            findings: Vec::new(),
            file_path,
            line_index,
            current_depth: 0,
            max_depth: 5, // Default threshold for nesting depth
        }
    }

    /// Checks if the current depth exceeds the maximum allowed depth.
    fn check_depth(&mut self, range_start: rustpython_ast::TextSize) {
        if self.current_depth > self.max_depth {
            let line = self.line_index.line_index(range_start);
            self.add_finding(
                &format!("Deeply nested code (depth {})", self.current_depth),
                "SKY-Q001",
                line,
            );
        }
    }

    /// Visits statements to track nesting depth.
    pub fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // Increase depth for function definitions
            Stmt::FunctionDef(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for async function definitions
            Stmt::AsyncFunctionDef(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for class definitions
            Stmt::ClassDef(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for If statements
            Stmt::If(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                // Note: We check orelse (else/elif) blocks but don't necessarily increase depth
                // relative to the `if` itself, but traversing them will naturally handle nested structures.
                // However, here we do increase depth for the *blocks* themselves if we consider `if` a block.
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for loops
            Stmt::For(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            Stmt::AsyncFor(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            Stmt::While(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for Try blocks
            Stmt::Try(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for handler in &node.handlers {
                    match handler {
                        ExceptHandler::ExceptHandler(h) => {
                            for stmt in &h.body {
                                self.visit_stmt(stmt);
                            }
                        }
                    }
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.finalbody {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            // Increase depth for With blocks
            Stmt::With(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            Stmt::AsyncWith(node) => {
                self.current_depth += 1;
                self.check_depth(node.range.start());
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                self.current_depth -= 1;
            }
            _ => {}
        }
    }

    /// Adds a finding to the list.
    /// Avoids duplicate findings for the same line and rule.
    fn add_finding(&mut self, msg: &str, rule_id: &str, line: usize) {
        if let Some(last) = self.findings.last() {
            if last.line == line && last.rule_id == rule_id {
                return;
            }
        }

        self.findings.push(QualityFinding {
            message: msg.to_string(),
            rule_id: rule_id.to_string(),
            file: self.file_path.clone(),
            line,
            severity: "LOW".to_string(),
        });
    }
}

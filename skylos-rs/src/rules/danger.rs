use crate::utils::LineIndex;
use rustpython_ast::{self as ast, Expr, Stmt};
use serde::Serialize;
use std::path::PathBuf;

/// Represents a security vulnerability finding.
#[derive(Debug, Clone, Serialize)]
pub struct DangerFinding {
    /// Description of the issue.
    pub message: String,
    /// Unique rule identifier (e.g., "SKY-D001").
    pub rule_id: String,
    /// File where the issue was found.
    pub file: PathBuf,
    /// Line number.
    pub line: usize,
    /// Severity level (e.g., "CRITICAL").
    pub severity: String,
}

/// Visitor that checks for dangerous code patterns.
///
/// This visitor looks for known security issues like `eval()`, `exec()`, or `subprocess` with `shell=True`.
pub struct DangerVisitor<'a> {
    /// Collected findings.
    pub findings: Vec<DangerFinding>,
    /// Current file path.
    pub file_path: PathBuf,
    /// Helper for line mapping.
    pub line_index: &'a LineIndex,
}

impl<'a> DangerVisitor<'a> {
    /// Creates a new `DangerVisitor`.
    pub fn new(file_path: PathBuf, line_index: &'a LineIndex) -> Self {
        Self {
            findings: Vec::new(),
            file_path,
            line_index,
        }
    }

    /// Visits statements to find dangerous patterns.
    pub fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(node) => self.visit_expr(&node.value),
            Stmt::FunctionDef(node) => {
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::ClassDef(node) => {
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            // Recurse for other statements if needed, currently simplified
            _ => {}
        }
    }

    /// Visits expressions to find dangerous function calls.
    pub fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(node) => {
                self.check_call(node);
                // Recursively check arguments
                self.visit_expr(&node.func);
                for arg in &node.args {
                    self.visit_expr(arg);
                }
            }
            _ => {}
        }
    }

    /// Checks a function call for security issues.
    fn check_call(&mut self, call: &ast::ExprCall) {
        if let Some(name) = self.get_call_name(&call.func) {
            let line = self.line_index.line_index(call.range.start());

            // SKY-D001: Avoid using eval/exec
            // These functions execute arbitrary code, which is a major security risk.
            if name == "eval" || name == "exec" {
                self.add_finding("Avoid using eval/exec", "SKY-D001", line);
            }

            // SKY-D002: subprocess with shell=True
            // This can lead to shell injection vulnerabilities if arguments are not sanitized.
            if name == "subprocess.call" || name == "subprocess.Popen" || name == "subprocess.run" {
                // Check for shell=True in keyword arguments
                for keyword in &call.keywords {
                    if let Some(arg) = &keyword.arg {
                        if arg == "shell" {
                            if let Expr::Constant(c) = &keyword.value {
                                if let ast::Constant::Bool(true) = c.value {
                                    self.add_finding(
                                        "subprocess with shell=True",
                                        "SKY-D002",
                                        line,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Extracts the function name from the call expression.
    fn get_call_name(&self, func: &Expr) -> Option<String> {
        match func {
            Expr::Name(node) => Some(node.id.to_string()),
            Expr::Attribute(node) => {
                if let Expr::Name(value) = &*node.value {
                    Some(format!("{}.{}", value.id, node.attr))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Adds a finding to the list.
    fn add_finding(&mut self, msg: &str, rule_id: &str, line: usize) {
        self.findings.push(DangerFinding {
            message: msg.to_string(),
            rule_id: rule_id.to_string(),
            file: self.file_path.clone(),
            line,
            severity: "CRITICAL".to_string(),
        });
    }
}

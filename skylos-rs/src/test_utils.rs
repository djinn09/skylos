use crate::utils::LineIndex;
use regex::Regex;
use rustpython_ast::{self as ast, Expr, Stmt};
use std::path::Path;

lazy_static::lazy_static! {
    // Match files in test/tests directories OR files ending with _test.py
    // This matches the Python version: r"(?:^|[/\\])tests?[/\\]|_test\.py$"
    // This regex is used to identify if a file is likely a test file.
    static ref TEST_FILE_RE: Regex = Regex::new(r"(?:^|[/\\])tests?[/\\]|_test\.py$").unwrap();
}

/// A visitor that detects test-related code.
///
/// This is important because "unused" code in test files (like helper functions or fixtures)
/// is often valid and shouldn't be reported as dead code.
pub struct TestAwareVisitor<'a> {
    /// Indicates if the file being visited is considered a test file based on its path/name.
    pub is_test_file: bool,
    /// List of line numbers that contain test functions or fixtures.
    /// Definitions on these lines will receive a confidence penalty (likely ignored).
    pub test_decorated_lines: Vec<usize>,
    /// Helper for mapping byte offsets to line numbers.
    pub line_index: &'a LineIndex,
}

impl<'a> TestAwareVisitor<'a> {
    /// Creates a new `TestAwareVisitor`.
    ///
    /// Determines if the file is a test file based on the file path.
    pub fn new(path: &Path, line_index: &'a LineIndex) -> Self {
        let path_str = path.to_string_lossy();
        // Check if the file path matches the test file regex.
        let is_test_file = TEST_FILE_RE.is_match(&path_str);

        Self {
            is_test_file,
            test_decorated_lines: Vec::new(),
            line_index,
        }
    }

    /// Visits statements to find test functions and classes.
    pub fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::FunctionDef(node) => {
                let name = &node.name;
                let line = self.line_index.line_index(node.range.start());

                // Heuristic: Functions starting with `test_` or ending with `_test` are likely tests.
                if name.starts_with("test_") || name.ends_with("_test") {
                    self.test_decorated_lines.push(line);
                }

                // Check decorators for pytest fixtures or markers.
                for decorator in &node.decorator_list {
                    if let Expr::Name(name_node) = decorator {
                        if name_node.id.contains("pytest") || name_node.id.contains("fixture") {
                            self.test_decorated_lines.push(line);
                        }
                    } else if let Expr::Attribute(attr_node) = decorator {
                        if attr_node.attr.contains("pytest") || attr_node.attr.contains("fixture") {
                            self.test_decorated_lines.push(line);
                        }
                    }
                }

                // Recurse into the function body.
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::ClassDef(node) => {
                let name = &node.name;
                // Heuristic: Classes named `Test...` or `...Test` are likely test suites.
                if name.starts_with("Test") || name.ends_with("Test") {
                    let line = self.line_index.line_index(node.range.start());
                    self.test_decorated_lines.push(line);
                }
                // Recurse into the class body.
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            _ => {}
        }
    }
}

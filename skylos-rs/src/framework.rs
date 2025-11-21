use crate::utils::LineIndex;
use rustpython_ast::{self as ast, Expr, Stmt};
use std::collections::HashSet;

/// Lazy static initialization for known framework modules.
/// These libraries are commonly used in Python web development and data processing.
/// Code using these frameworks often has implicit usage patterns (e.g., dependency injection).
lazy_static::lazy_static! {
    static ref FRAMEWORK_IMPORTS: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("flask");
        s.insert("fastapi");
        s.insert("django");
        s.insert("rest_framework");
        s.insert("pydantic");
        s.insert("celery");
        s.insert("starlette");
        s.insert("uvicorn");
        s
    };
}

/// A visitor that detects framework usage in a Python file.
///
/// Frameworks often use decorators or inheritance to register components.
/// This visitor helps Skylos understand that some code might appear unused but is actually
/// used by the framework (e.g., a route handler).
pub struct FrameworkAwareVisitor<'a> {
    /// Indicates if the current file uses a known framework.
    pub is_framework_file: bool,
    /// Set of detected frameworks in the file.
    pub detected_frameworks: HashSet<String>,
    /// Lines where framework-specific decorators are applied.
    /// Definitions on these lines receive a confidence penalty (are less likely to be reported as unused).
    pub framework_decorated_lines: HashSet<usize>,
    /// Helper for mapping byte offsets to line numbers.
    pub line_index: &'a LineIndex,
}

impl<'a> FrameworkAwareVisitor<'a> {
    /// Creates a new `FrameworkAwareVisitor`.
    pub fn new(line_index: &'a LineIndex) -> Self {
        Self {
            is_framework_file: false,
            detected_frameworks: HashSet::new(),
            framework_decorated_lines: HashSet::new(),
            line_index,
        }
    }

    /// Visits a statement to check for framework patterns.
    pub fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // Check imports to detect framework usage.
            Stmt::Import(node) => {
                for alias in &node.names {
                    let name = alias.name.as_str();
                    // Check if the imported module is a known framework.
                    for fw in FRAMEWORK_IMPORTS.iter() {
                        if name.contains(fw) {
                            self.is_framework_file = true;
                            self.detected_frameworks.insert(fw.to_string());
                        }
                    }
                }
            }
            // Check 'from ... import' statements.
            Stmt::ImportFrom(node) => {
                if let Some(module) = &node.module {
                    // Extract the base module name.
                    let module_name = module.split('.').next().unwrap_or("");
                    if FRAMEWORK_IMPORTS.contains(module_name) {
                        self.is_framework_file = true;
                        self.detected_frameworks.insert(module_name.to_string());
                    }
                }
            }
            // Check function definitions for decorators.
            Stmt::FunctionDef(node) => {
                let line = self.line_index.line_index(node.range.start());
                self.check_decorators(&node.decorator_list, line);
                // Recursively visit the body of the function.
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            // Check class definitions for base classes and content.
            Stmt::ClassDef(node) => {
                // Check base classes (inheritance) for framework patterns.
                // e.g., inheriting from `Model`, `View`, `Schema`.
                for base in &node.bases {
                    if let Expr::Name(name_node) = base {
                        let id = name_node.id.to_lowercase();
                        if id.contains("view") || id.contains("model") || id.contains("schema") {
                            self.is_framework_file = true;
                            // Mark this class as framework-related.
                            let line = self.line_index.line_index(node.range.start());
                            self.framework_decorated_lines.insert(line);
                        }
                    }
                }

                // Recursively visit the body of the class.
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            _ => {}
        }
    }

    /// Checks if any of the decorators are framework-related.
    fn check_decorators(&mut self, decorators: &[Expr], line: usize) {
        for decorator in decorators {
            let name = self.get_decorator_name(decorator);
            if self.is_framework_decorator(&name) {
                // If a framework decorator is found, mark the line and the file.
                self.framework_decorated_lines.insert(line);
                self.is_framework_file = true;
            }
        }
    }

    /// Extracts the name of a decorator.
    fn get_decorator_name(&self, decorator: &Expr) -> String {
        match decorator {
            Expr::Name(node) => node.id.to_string(),
            Expr::Attribute(node) => {
                // For decorators like @app.route
                node.attr.to_string()
            }
            Expr::Call(node) => {
                // For decorators with arguments like @app.route("/path")
                self.get_decorator_name(&node.func)
            }
            _ => String::new(),
        }
    }

    /// Determines if a decorator name is likely framework-related.
    fn is_framework_decorator(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        // Common patterns in Flask, FastAPI, Celery, etc.
        name.contains("route")
            || name.contains("get")
            || name.contains("post")
            || name.contains("put")
            || name.contains("delete")
            || name.contains("validator")
            || name.contains("task") // celery
    }
}

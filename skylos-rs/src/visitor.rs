use crate::utils::LineIndex;
use rustpython_ast::{self as ast, Expr, Stmt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a defined entity (function, class, variable, import) in the Python code.
/// This struct holds metadata about the definition, including its location and confidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    /// The name of the defined entity (e.g., "my_function").
    pub name: String,
    /// The fully qualified name (e.g., "module.class.method").
    pub full_name: String,
    /// The simple name (last part of the full name).
    pub simple_name: String,
    /// The type of definition ("function", "class", "method", "import", "variable").
    pub def_type: String,
    /// The file path where this definition resides.
    pub file: PathBuf,
    /// The line number where this definition starts.
    pub line: usize,
    /// A confidence score (0-100) indicating how certain we are that this is unused.
    /// Higher means more likely to be a valid finding.
    pub confidence: u8,
    /// The number of times this definition is referenced in the codebase.
    pub references: usize,
    /// Whether this definition is considered exported (implicitly used).
    pub is_exported: bool,
    /// Whether this definition is inside an `__init__.py` file.
    pub in_init: bool,
    /// List of base classes if this is a class definition.
    pub base_classes: Vec<String>,
}

impl Definition {
    /// Apply confidence penalties based on naming patterns and context.
    ///
    /// This adjusts the `confidence` score to reduce false positives.
    /// For example, private methods or dunder methods are often implicitly used,
    /// so we lower the confidence that they are "unused" even if we don't see explicit references.
    pub fn apply_penalties(&mut self) {
        let mut confidence: i16 = 100;

        // Private names (starts with _ but not __)
        // These are internal and might be used via dynamic access or just be implementation details.
        if self.simple_name.starts_with('_') && !self.simple_name.starts_with("__") {
            confidence -= 30;
        }

        // Dunder/magic methods - zero confidence
        // Python calls these implicitly (e.g., `__init__`, `__str__`).
        if self.simple_name.starts_with("__") && self.simple_name.ends_with("__") {
            confidence = 0;
        }

        // In __init__.py penalty
        // Functions and classes in `__init__.py` are often there to be exported by the package,
        // so we assume they might be used externally.
        if self.in_init && (self.def_type == "function" || self.def_type == "class") {
            confidence -= 20;
        }

        self.confidence = confidence.max(0) as u8;
    }
}

/// The main visitor for collecting definitions and references from the AST.
pub struct SkylosVisitor<'a> {
    /// Collected definitions.
    pub definitions: Vec<Definition>,
    /// Collected references (name usage).
    pub references: Vec<(String, PathBuf)>,
    /// Names explicitly exported via `__all__`.
    pub exports: Vec<String>,
    /// Dynamic imports detected.
    pub dynamic_imports: Vec<String>,
    /// The path of the file being visited.
    pub file_path: PathBuf,
    /// The module name derived from the file path.
    pub module_name: String,
    /// Current scope stack (not fully used currently but good for tracking nested scopes).
    pub current_scope: Vec<String>,
    /// Stack of class names to track current class context.
    pub class_stack: Vec<String>,
    /// Helper for line number mapping.
    pub line_index: &'a LineIndex,
}

impl<'a> SkylosVisitor<'a> {
    /// Creates a new `SkylosVisitor`.
    pub fn new(file_path: PathBuf, module_name: String, line_index: &'a LineIndex) -> Self {
        Self {
            definitions: Vec::new(),
            references: Vec::new(),
            exports: Vec::new(),
            dynamic_imports: Vec::new(),
            file_path,
            module_name,
            current_scope: Vec::new(),
            class_stack: Vec::new(),
            line_index,
        }
    }

    /// Helper to add a definition with default parameters.
    fn add_def(&mut self, name: String, def_type: &str, line: usize) {
        self.add_def_with_bases(name, def_type, line, Vec::new());
    }

    /// Adds a definition to the list, applying heuristics for implicit usage.
    fn add_def_with_bases(
        &mut self,
        name: String,
        def_type: &str,
        line: usize,
        base_classes: Vec<String>,
    ) {
        let simple_name = name.split('.').last().unwrap_or(&name).to_string();
        let in_init = self.file_path.ends_with("__init__.py");

        // GENERIC HEURISTICS (No hardcoded project names)

        // 1. Tests: Functions starting with 'test_' are assumed to be Pytest/Unittest tests.
        // These are run by test runners, not called explicitly.
        let is_test = simple_name.starts_with("test_");

        // 2. Dynamic Dispatch Patterns:
        //    - 'visit_' / 'leave_': Standard Visitor pattern (AST, LibCST)
        //    - 'on_': Standard Event Handler pattern (UI libs, callbacks)
        let is_dynamic_pattern = simple_name.starts_with("visit_")
            || simple_name.starts_with("leave_")
            || simple_name.starts_with("on_");

        // 3. Standard Entry Points: Common names for script execution.
        let is_standard_entry = matches!(simple_name.as_str(), "main" | "run" | "execute");

        // 4. Dunder Methods: Python's magic methods (__str__, __init__, etc.) are implicitly used.
        let is_dunder = simple_name.starts_with("__") && simple_name.ends_with("__");

        // Decision: Is this implicitly used/exported?
        let is_implicitly_used = is_test || is_dynamic_pattern || is_standard_entry || is_dunder;

        // Set reference count to 1 if implicitly used to prevent false positives.
        // This treats the definition as "used".
        let references = if is_implicitly_used { 1 } else { 0 };

        let definition = Definition {
            name: name.clone(),
            full_name: name,
            simple_name,
            def_type: def_type.to_string(),
            file: self.file_path.clone(),
            line,
            confidence: 100,
            references,
            is_exported: is_implicitly_used,
            in_init,
            base_classes,
        };

        self.definitions.push(definition);
    }

    /// Records a reference to a name.
    pub fn add_ref(&mut self, name: String) {
        self.references.push((name, self.file_path.clone()));
    }

    /// Constructs a qualified name based on the current module and class stack.
    fn get_qualified_name(&self, name: &str) -> String {
        let mut parts = Vec::new();
        if !self.module_name.is_empty() {
            parts.push(self.module_name.clone());
        }
        parts.extend(self.class_stack.clone());
        parts.push(name.to_string());
        parts.join(".")
    }

    /// Visits a statement node in the AST.
    pub fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // Handle function definitions
            Stmt::FunctionDef(node) => {
                self.visit_function_def(&node.name, &node.body, node.range.start());
            }
            // Handle async function definitions
            Stmt::AsyncFunctionDef(node) => {
                self.visit_function_def(&node.name, &node.body, node.range.start());
            }
            // Handle class definitions
            Stmt::ClassDef(node) => {
                let name = &node.name;
                let qualified_name = self.get_qualified_name(name.as_str());
                let line = self.line_index.line_index(node.range.start());

                // Extract base class names to check for inheritance patterns later.
                let mut base_classes = Vec::new();
                for base in &node.bases {
                    match base {
                        Expr::Name(base_name) => {
                            base_classes.push(base_name.id.to_string());
                        }
                        Expr::Attribute(attr) => {
                            base_classes.push(attr.attr.to_string());
                        }
                        _ => {}
                    }
                }

                self.add_def_with_bases(qualified_name, "class", line, base_classes.clone());

                // Add references for base classes because inheriting uses them.
                for base in &node.bases {
                    self.visit_expr(base);
                    // Handle simple base class names mapping to module refs
                    if let Expr::Name(base_name) = base {
                        self.add_ref(base_name.id.to_string()); // Also add simple reference
                        if !self.module_name.is_empty() {
                            let qualified_base = format!("{}.{}", self.module_name, base_name.id);
                            self.add_ref(qualified_base);
                        }
                    }
                }

                // Push class name to stack for nested definitions (methods/inner classes).
                self.class_stack.push(name.to_string());
                // Visit class body.
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                // Pop class name after visiting body.
                self.class_stack.pop();
            }
            // Handle imports
            Stmt::Import(node) => {
                for alias in &node.names {
                    let asname = alias.asname.as_ref().unwrap_or(&alias.name);
                    let line = self.line_index.line_index(node.range.start());
                    self.add_def(asname.to_string(), "import", line);
                }
            }
            // Handle 'from ... import'
            Stmt::ImportFrom(node) => {
                // FIX: Ignore __future__ imports to stop "unused import annotations"
                // `from __future__ import ...` is a compiler directive, not a real import.
                if let Some(module) = &node.module {
                    if module == "__future__" {
                        // Skip adding definitions for future imports
                        return;
                    }
                }

                let line = self.line_index.line_index(node.range.start());
                for alias in &node.names {
                    let asname = alias.asname.as_ref().unwrap_or(&alias.name);
                    self.add_def(asname.to_string(), "import", line);
                }
            }
            // Handle assignments
            Stmt::Assign(node) => {
                // Handle __all__ exports. `__all__ = ["a", "b"]` explicitly exports names.
                if let Some(Expr::Name(target)) = node.targets.first() {
                    if target.id.as_str() == "__all__" {
                        if let Expr::List(list) = &*node.value {
                            for elt in &list.elts {
                                if let Expr::Constant(constant) = elt {
                                    if let ast::Constant::Str(s) = &constant.value {
                                        self.exports.push(s.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                self.visit_expr(&node.value);
            }
            // Handle expression statements
            Stmt::Expr(node) => {
                self.visit_expr(&node.value);
            }
            // Control Flow Handling - traverse bodies recursively
            Stmt::If(node) => {
                self.visit_expr(&node.test);
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::For(node) => {
                self.visit_expr(&node.iter);
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::AsyncFor(node) => {
                self.visit_expr(&node.iter);
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::While(node) => {
                self.visit_expr(&node.test);
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::With(node) => {
                for item in &node.items {
                    self.visit_expr(&item.context_expr);
                }
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::AsyncWith(node) => {
                for item in &node.items {
                    self.visit_expr(&item.context_expr);
                }
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::Try(node) => {
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for handler in &node.handlers {
                    // Fix: Unwrap the Excepthandler enum
                    if let ast::ExceptHandler::ExceptHandler(handler_node) = handler {
                        if let Some(exc) = &handler_node.type_ {
                            self.visit_expr(exc);
                        }
                        for stmt in &handler_node.body {
                            self.visit_stmt(stmt);
                        }
                    }
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.finalbody {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::TryStar(node) => {
                for stmt in &node.body {
                    self.visit_stmt(stmt);
                }
                for handler in &node.handlers {
                    // Fix: Unwrap the Excepthandler enum
                    if let ast::ExceptHandler::ExceptHandler(handler_node) = handler {
                        if let Some(exc) = &handler_node.type_ {
                            self.visit_expr(exc);
                        }
                        for stmt in &handler_node.body {
                            self.visit_stmt(stmt);
                        }
                    }
                }
                for stmt in &node.orelse {
                    self.visit_stmt(stmt);
                }
                for stmt in &node.finalbody {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::Return(node) => {
                if let Some(value) = &node.value {
                    self.visit_expr(value);
                }
            }
            _ => {}
        }
    }

    // Helper function to handle shared logic between FunctionDef and AsyncFunctionDef
    fn visit_function_def(
        &mut self,
        name: &str,
        body: &[Stmt],
        range_start: rustpython_ast::TextSize,
    ) {
        let qualified_name = self.get_qualified_name(name);
        let line = self.line_index.line_index(range_start);

        // Determine if it's a function or a method based on class stack.
        let def_type = if !self.class_stack.is_empty() {
            "method"
        } else {
            "function"
        };

        self.add_def(qualified_name, def_type, line);

        for stmt in body {
            self.visit_stmt(stmt);
        }
    }

    /// Visits an expression node in the AST.
    pub fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            // Name usage (variable access)
            Expr::Name(node) => {
                if node.ctx.is_load() {
                    self.add_ref(node.id.to_string());
                }
            }
            // Function call
            Expr::Call(node) => {
                self.visit_expr(&node.func);
                for arg in &node.args {
                    self.visit_expr(arg);
                }
                // Don't forget keyword arguments (e.g., func(a=b))
                for keyword in &node.keywords {
                    self.visit_expr(&keyword.value);
                }
            }
            // Attribute access (e.g., obj.attr)
            Expr::Attribute(node) => {
                if let Expr::Name(name_node) = &*node.value {
                    let base_id = name_node.id.as_str();

                    // Case 1: Strict self.method usage inside a class context.
                    // We want to track references to methods of the current class.
                    if (base_id == "self" || base_id == "cls") && !self.class_stack.is_empty() {
                        let method_name = &node.attr;
                        let mut parts = Vec::new();
                        if !self.module_name.is_empty() {
                            parts.push(self.module_name.clone());
                        }
                        parts.extend(self.class_stack.clone());
                        parts.push(method_name.to_string());
                        let qualified = parts.join(".");
                        self.add_ref(qualified);
                    }
                    // Case 2: External usage (obj.method or sys.exit)
                    else {
                        // Track "sys" from "sys.exit" (Fixes unused import)
                        self.add_ref(base_id.to_string());

                        // Track "sys.exit" (Specific attribute access)
                        let full_attr = format!("{}.{}", base_id, node.attr);
                        self.add_ref(full_attr);

                        // FIX: Loose Method Tracking
                        // Track "analyze" from "s.analyze()".
                        // This fixes "unused function" when we can't infer the type of 's'.
                        self.add_ref(node.attr.to_string());
                    }
                }
                self.visit_expr(&node.value);
            }
            // FIX: Dynamic Dispatch / String References
            Expr::Constant(node) => {
                if let ast::Constant::Str(s) = &node.value {
                    // Heuristic: If a string looks like a simple identifier (no spaces/dots),
                    // track it as a reference. This helps with getattr(self, "visit_" + name).
                    if !s.contains(' ') && !s.contains('.') && !s.is_empty() {
                        self.add_ref(s.to_string());
                    }
                }
            }
            // Recursion Boilerplate - Ensure we visit children of all other expressions
            Expr::BoolOp(node) => {
                for value in &node.values {
                    self.visit_expr(value);
                }
            }
            Expr::BinOp(node) => {
                self.visit_expr(&node.left);
                self.visit_expr(&node.right);
            }
            Expr::UnaryOp(node) => {
                self.visit_expr(&node.operand);
            }
            Expr::Lambda(node) => {
                self.visit_expr(&node.body);
            }
            Expr::IfExp(node) => {
                self.visit_expr(&node.test);
                self.visit_expr(&node.body);
                self.visit_expr(&node.orelse);
            }
            Expr::Dict(node) => {
                for (key, value) in node.keys.iter().zip(&node.values) {
                    if let Some(k) = key {
                        self.visit_expr(k);
                    }
                    self.visit_expr(value);
                }
            }
            Expr::Set(node) => {
                for elt in &node.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::ListComp(node) => {
                self.visit_expr(&node.elt);
                for gen in &node.generators {
                    self.visit_expr(&gen.iter);
                    for if_expr in &gen.ifs {
                        self.visit_expr(if_expr);
                    }
                }
            }
            Expr::SetComp(node) => {
                self.visit_expr(&node.elt);
                for gen in &node.generators {
                    self.visit_expr(&gen.iter);
                    for if_expr in &gen.ifs {
                        self.visit_expr(if_expr);
                    }
                }
            }
            Expr::DictComp(node) => {
                self.visit_expr(&node.key);
                self.visit_expr(&node.value);
                for gen in &node.generators {
                    self.visit_expr(&gen.iter);
                    for if_expr in &gen.ifs {
                        self.visit_expr(if_expr);
                    }
                }
            }
            Expr::GeneratorExp(node) => {
                self.visit_expr(&node.elt);
                for gen in &node.generators {
                    self.visit_expr(&gen.iter);
                    for if_expr in &gen.ifs {
                        self.visit_expr(if_expr);
                    }
                }
            }
            Expr::Await(node) => self.visit_expr(&node.value),
            Expr::Yield(node) => {
                if let Some(value) = &node.value {
                    self.visit_expr(value);
                }
            }
            Expr::YieldFrom(node) => self.visit_expr(&node.value),
            Expr::Compare(node) => {
                self.visit_expr(&node.left);
                for comparator in &node.comparators {
                    self.visit_expr(comparator);
                }
            }
            Expr::Subscript(node) => {
                self.visit_expr(&node.value);
                self.visit_expr(&node.slice);
            }
            Expr::FormattedValue(node) => self.visit_expr(&node.value),
            Expr::JoinedStr(node) => {
                for value in &node.values {
                    self.visit_expr(value);
                }
            }
            Expr::List(node) => {
                for elt in &node.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::Tuple(node) => {
                for elt in &node.elts {
                    self.visit_expr(elt);
                }
            }
            Expr::Slice(node) => {
                if let Some(lower) = &node.lower {
                    self.visit_expr(lower);
                }
                if let Some(upper) = &node.upper {
                    self.visit_expr(upper);
                }
                if let Some(step) = &node.step {
                    self.visit_expr(step);
                }
            }
            _ => {}
        }
    }
}

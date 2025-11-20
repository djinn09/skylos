//! Example: Debug Visitor
//!
//! This example demonstrates how to use the `SkylosVisitor` directly to parse Python code
//! and extract definitions and references. It is useful for debugging the AST traversal logic.

use rustpython_parser::{parse, Mode};
use skylos_rs::utils::LineIndex;
use skylos_rs::visitor::SkylosVisitor;
use std::path::PathBuf;

/// Main entry point for the debug example.
fn main() {
    // Sample Python source code with classes and methods (instance and class methods).
    let source = r#"
class BaseClass:
    pass

class ChildClass(BaseClass):
    def instance_method(self):
        self.helper()

    def class_method(cls):
        cls.static_helper()
        
    def helper(self):
        pass
        
    def static_helper(cls):
        pass
"#;

    // Create a LineIndex to map byte offsets to line numbers.
    // This is needed for reporting findings with correct line numbers.
    let line_index = LineIndex::new(source);

    // Initialize the visitor.
    // We simulate analyzing a file named "test_parity.py" in a module named "test_parity".
    let mut visitor = SkylosVisitor::new(
        PathBuf::from("test_parity.py"),
        "test_parity".to_string(),
        &line_index,
    );

    // Parse the Python source code into an AST.
    // We unwrap directly as this is just an example/debug script.
    let ast = parse(source, Mode::Module, "test_parity.py").unwrap();

    // Traverse the AST.
    // We verify that the parsed result is a Module and iterate over its statements.
    if let rustpython_ast::Mod::Module(module) = ast {
        for stmt in module.body {
            visitor.visit_stmt(&stmt);
        }
    }

    // Print all found definitions (classes, functions, methods).
    println!("=== DEFINITIONS ===");
    for def in &visitor.definitions {
        println!("{}: {} (line {})", def.name, def.def_type, def.line);
    }

    // Print all found references (variable usage, function calls).
    println!("\n=== REFERENCES ===");
    for (ref_name, _) in &visitor.references {
        println!("{}", ref_name);
    }
}

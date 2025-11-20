// Unit tests for quality rules
// Tests code quality checks like nesting depth

use skylos_rs::rules::quality::QualityVisitor;
use skylos_rs::utils::LineIndex;
use rustpython_parser::{parse, Mode};
use std::path::PathBuf;

#[test]
fn test_deeply_nested_code_detection() {
    let source = r#"
def deeply_nested():
    if True:
        if True:
            if True:
                if True:
                    if True:
                        if True:
                            print("too deep")
"#;
    
    let tree = parse(source, Mode::Module, "test.py").expect("Failed to parse");
    let line_index = LineIndex::new(source);
    let mut visitor = QualityVisitor::new(PathBuf::from("test.py"), &line_index);
    
    if let rustpython_ast::Mod::Module(module) = tree {
        for stmt in &module.body {
            visitor.visit_stmt(stmt);
        }
    }
    
    assert!(visitor.findings.len() > 0, "Should detect deeply nested code");
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-Q001"));
}

#[test]
fn test_acceptable_nesting() {
    let source = r#"
def normal_function():
    if True:
        for item in range(10):
            print(item)
"#;
    
    let tree = parse(source, Mode::Module, "test.py").expect("Failed to parse");
    let line_index = LineIndex::new(source);
    let mut visitor = QualityVisitor::new(PathBuf::from("test.py"), &line_index);
    
    if let rustpython_ast::Mod::Module(module) = tree {
        for stmt in &module.body {
            visitor.visit_stmt(stmt);
        }
    }
    
    // Should not flag normal nesting (depth <= 5)
    assert_eq!(visitor.findings.len(), 0, "Should not flag acceptable nesting");
}

use skylos_rs::analyzer::Skylos;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_analyze_basic() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.py");
    let mut file = File::create(&file_path).unwrap();

    let content = r#"
def used_function():
    return "used"

def unused_function():
    return "unused"

class UsedClass:
    def method(self):
        pass

class UnusedClass:
    def method(self):
        pass

result = used_function()
instance = UsedClass()
"#;
    write!(file, "{}", content).unwrap();

    let skylos = Skylos::new(60, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    // Verify unused functions
    let unused_funcs: Vec<String> = result
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();
    assert!(unused_funcs.contains(&"unused_function".to_string()));
    assert!(!unused_funcs.contains(&"used_function".to_string()));

    // Verify unused classes
    let unused_classes: Vec<String> = result
        .unused_classes
        .iter()
        .map(|c| c.simple_name.clone())
        .collect();
    assert!(unused_classes.contains(&"UnusedClass".to_string()));
    assert!(!unused_classes.contains(&"UsedClass".to_string()));

    // Verify summary
    assert_eq!(result.analysis_summary.total_files, 1);
}

#[test]
fn test_analyze_empty_directory() {
    let dir = tempdir().unwrap();
    let skylos = Skylos::new(60, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    assert_eq!(result.analysis_summary.total_files, 0);
    assert!(result.unused_functions.is_empty());
    assert!(result.unused_classes.is_empty());
}

#[test]
fn test_confidence_threshold_filtering() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.py");
    let mut file = File::create(&file_path).unwrap();

    // _private is penalized, so its confidence should be lower
    let content = r#"
def regular_unused():
    pass

def _private_unused():
    pass
"#;
    write!(file, "{}", content).unwrap();

    // High threshold: _private_unused should be filtered out (low confidence)
    // regular_unused (100) should remain
    // _private_unused (100 - 30 = 70)

    // Set threshold to 80
    let skylos_high = Skylos::new(80, false, false, false);
    let result_high = skylos_high.analyze(dir.path()).unwrap();

    let funcs_high: Vec<String> = result_high
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();

    assert!(funcs_high.contains(&"regular_unused".to_string()));
    assert!(!funcs_high.contains(&"_private_unused".to_string()));

    // Low threshold: both should be present
    let skylos_low = Skylos::new(60, false, false, false);
    let result_low = skylos_low.analyze(dir.path()).unwrap();

    let funcs_low: Vec<String> = result_low
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();

    assert!(funcs_low.contains(&"regular_unused".to_string()));
    assert!(funcs_low.contains(&"_private_unused".to_string()));
}

#[test]
fn test_module_name_generation_implicit() {
    let dir = tempdir().unwrap();

    // Create src/package/submodule.py
    let package_path = dir.path().join("src").join("package");
    fs::create_dir_all(&package_path).unwrap();

    let file_path = package_path.join("submodule.py");
    let mut file = File::create(&file_path).unwrap();
    write!(file, "def test_func(): pass").unwrap();

    let skylos = Skylos::new(0, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    // We can't check internal module name directly, but we can check if full_name reflects it?
    // In Rust impl, module name is just file_stem (e.g. "submodule"), not dotted path "src.package.submodule"
    // So the full name would be "submodule.test_func" or "test_func" if module name is ignored in some contexts.
    // Let's check what we get.

    if let Some(func) = result.unused_functions.first() {
        // Based on analyzer.rs: let module_name = path.file_stem()
        // It creates "submodule"
        assert_eq!(func.full_name, "submodule.test_func");
    } else {
        panic!("No unused function found");
    }
}

#[test]
fn test_heuristics_auto_called_methods() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.py");
    let mut file = File::create(&file_path).unwrap();

    let content = r#"
class MyClass:
    def __init__(self):
        pass

    def __str__(self):
        return "string"

instance = MyClass()
"#;
    write!(file, "{}", content).unwrap();

    let skylos = Skylos::new(0, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    // __init__ and __str__ are dunder methods, confidence should be 0 (or penalized to point of exclusion if threshold is high)
    // In Rust impl: dunder methods get confidence 0.
    // If confidence is 0, they should still appear if threshold is 0.
    // Wait, Skylos::analyze filters: if def.confidence < self.confidence_threshold { continue; }
    // If threshold is 0, and confidence is 0, is 0 < 0? No.
    // So they should appear if confidence >= threshold.
    // If confidence is 0 and threshold is 0, they appear.

    let unused_funcs: Vec<String> = result
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();

    // They are unused in terms of references (0 refs), but confidence 0.
    // If we want to verify they are detected but penalized:
    assert!(unused_funcs.contains(&"__init__".to_string()));
    assert!(unused_funcs.contains(&"__str__".to_string()));

    // Verify confidence is 0
    let init_def = result
        .unused_functions
        .iter()
        .find(|f| f.simple_name == "__init__")
        .unwrap();
    assert_eq!(init_def.confidence, 0);
}

#[test]
fn test_mark_exports_in_init() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("__init__.py");
    let mut file = File::create(&file_path).unwrap();

    let content = r#"
def public_function():
    pass

def _private_function():
    pass
"#;
    write!(file, "{}", content).unwrap();

    let skylos = Skylos::new(0, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    // In Rust impl: "In __init__.py penalty ... confidence -= 20"
    // And "Private names ... confidence -= 30"

    let public_def = result
        .unused_functions
        .iter()
        .find(|f| f.simple_name == "public_function")
        .unwrap();
    assert_eq!(public_def.in_init, true);
    // Base 100 - 20 = 80
    assert_eq!(public_def.confidence, 80);

    let private_def = result
        .unused_functions
        .iter()
        .find(|f| f.simple_name == "_private_function")
        .unwrap();
    // Base 100 - 30 (private) - 20 (init) = 50
    assert_eq!(private_def.confidence, 50);
}

#[test]
fn test_mark_refs_direct_reference() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.py");
    let mut file = File::create(&file_path).unwrap();

    let content = r#"
def my_func():
    pass

my_func()
"#;
    write!(file, "{}", content).unwrap();

    let skylos = Skylos::new(0, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    let unused_funcs: Vec<String> = result
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();

    assert!(!unused_funcs.contains(&"my_func".to_string()));
}

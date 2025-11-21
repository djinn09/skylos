use skylos_rs::analyzer::Skylos;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_analyze_respects_ignore_pragmas() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("demo.py");
    let mut file = File::create(&file_path).unwrap();

    let content = r#"
def used():
    pass

def unused_no_ignore():
    pass

def unused_ignore():   # pragma: no skylos
    pass

used()
"#;
    write!(file, "{}", content).unwrap();

    let skylos = Skylos::new(0, false, false, false);
    let result = skylos.analyze(dir.path()).unwrap();

    let unreachable: Vec<String> = result
        .unused_functions
        .iter()
        .map(|f| f.simple_name.clone())
        .collect();

    assert!(unreachable.contains(&"unused_no_ignore".to_string()));
    assert!(!unreachable.contains(&"unused_ignore".to_string()));
    assert!(!unreachable.contains(&"used".to_string()));
}

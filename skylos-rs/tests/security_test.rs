// Unit tests for security rules
// Tests secrets and dangerous code detection

use rustpython_parser::{parse, Mode};
use skylos_rs::rules::danger::DangerVisitor;
use skylos_rs::rules::secrets::scan_secrets;
use skylos_rs::utils::LineIndex;
use std::path::PathBuf;

// --- DANGER TESTS ---

macro_rules! scan_danger {
    ($source:expr, $visitor:ident) => {
        let tree = parse($source, Mode::Module, "test.py").expect("Failed to parse");
        let line_index = LineIndex::new($source);
        let mut $visitor = DangerVisitor::new(PathBuf::from("test.py"), &line_index);

        if let rustpython_ast::Mod::Module(module) = tree {
            for stmt in &module.body {
                $visitor.visit_stmt(stmt);
            }
        }
    };
}

#[test]
fn test_eval_detection() {
    let source = r#"
user_input = input("Enter code: ")
result = eval(user_input)
"#;
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D201"));
}

#[test]
fn test_exec_detection() {
    let source = r#"
code = "print('hello')"
exec(code)
"#;
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D202"));
}

#[test]
fn test_os_system() {
    let source = "import os\nos.system('echo hi')\n";
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D203"));
}

#[test]
fn test_pickle_loads() {
    let source = "import pickle\npickle.loads(b'\\x80\\x04K\\x01.')\n";
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D205"));
}

#[test]
fn test_yaml_load_without_safeloader() {
    let source = "import yaml\nyaml.load('a: 1')\n";
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D206"));
}

#[test]
fn test_md5_sha1() {
    let source = "import hashlib\nhashlib.md5(b'd')\nhashlib.sha1(b'd')\n";
    scan_danger!(source, visitor);
    let ids: Vec<_> = visitor.findings.iter().map(|f| &f.rule_id).collect();
    assert!(ids.contains(&&"SKY-D207".to_string()));
    assert!(ids.contains(&&"SKY-D208".to_string()));
}

#[test]
fn test_subprocess_shell_true() {
    let source = "import subprocess\nsubprocess.run('echo hi', shell=True)\n";
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D209"));
}

#[test]
fn test_requests_verify_false() {
    let source = "import requests\nrequests.get('https://x', verify=False)\n";
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D210"));
}

#[test]
fn test_yaml_safe_loader_does_not_trigger() {
    let source = "import yaml\nfrom yaml import SafeLoader\nyaml.load('a: 1', Loader=SafeLoader)\n";
    scan_danger!(source, visitor);
    assert!(!visitor.findings.iter().any(|f| f.rule_id == "SKY-D206"));
}

#[test]
fn test_subprocess_without_shell_true_is_ok() {
    let source = "import subprocess\nsubprocess.run(['echo','hi'])\n";
    scan_danger!(source, visitor);
    assert!(!visitor.findings.iter().any(|f| f.rule_id == "SKY-D209"));
}

#[test]
fn test_requests_default_verify_true_is_ok() {
    let source = "import requests\nrequests.get('https://example.com')\n";
    scan_danger!(source, visitor);
    assert!(!visitor.findings.iter().any(|f| f.rule_id == "SKY-D210"));
}

#[test]
fn test_sql_execute_interpolated_flags() {
    let source = r#"
def f(cur, name):
    # f-string interpolation -> should flag SKY-D211
    cur.execute(f"SELECT * FROM users WHERE name = '{name}'")
"#;
    scan_danger!(source, visitor);
    assert!(visitor.findings.iter().any(|f| f.rule_id == "SKY-D211"));
}

#[test]
fn test_sql_execute_parameterized_ok() {
    let source = r#"
def f(cur, name):
    cur.execute("SELECT * FROM users WHERE name = %s", (name,))
"#;
    scan_danger!(source, visitor);
    assert!(!visitor.findings.iter().any(|f| f.rule_id == "SKY-D211"));
}

// --- SECRETS TESTS ---

#[test]
fn test_aws_key_detection() {
    let source = r#"
AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE"
AWS_SECRET_ACCESS_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
"#;
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.contains("AWS Access Key")));
}

#[test]
fn test_github_token_detection() {
    let source = "GITHUB_TOKEN = \"ghp_1234567890abcdef1234567890abcdef1234\"\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.to_lowercase().contains("github")));
}

#[test]
fn test_gitlab_pat_detection() {
    let source = "GITLAB_PAT = \"glpat-A1b2C3d4E5f6G7h8I9j0\"\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.to_lowercase().contains("gitlab")));
}

#[test]
fn test_slack_bot_detection() {
    let source = "SLACK_BOT = \"xoxb-1234567890ABCDEF12\"\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.to_lowercase().contains("slack")));
}

#[test]
fn test_stripe_key_detection() {
    let source = "STRIPE = \"sk_live_a1B2c3D4e5F6g7H8\"\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.to_lowercase().contains("stripe")));
}

#[test]
fn test_private_key_detection() {
    let source = "PK = \"-----BEGIN RSA PRIVATE KEY-----\"\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings
        .iter()
        .any(|f| f.message.to_lowercase().contains("private key")));
}

#[test]
fn test_ignore_directive_suppresses_matches() {
    let source =
        "GITHUB_TOKEN = \"ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"  # pragma: no skylos\n";
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert!(findings.is_empty());
}

#[test]
fn test_no_secrets_in_clean_code() {
    let source = r#"
def calculate(x, y):
    return x + y

API_URL = "https://api.example.com"
"#;
    let findings = scan_secrets(source, &PathBuf::from("test.py"));
    assert_eq!(findings.len(), 0);
}

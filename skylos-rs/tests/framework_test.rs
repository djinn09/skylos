// Unit tests for framework awareness
// Tests detection of Flask, Django, FastAPI patterns

use rustpython_parser::{parse, Mode};
use skylos_rs::framework::FrameworkAwareVisitor;
use skylos_rs::utils::LineIndex;

// Helper macro to avoid lifetime issues with returning visitor borrowing local line_index
macro_rules! scan_framework {
    ($source:expr, $visitor:ident) => {
        let tree = parse($source, Mode::Module, "test.py").expect("Failed to parse");
        let line_index = LineIndex::new($source);
        let mut $visitor = FrameworkAwareVisitor::new(&line_index);

        if let rustpython_ast::Mod::Module(module) = tree {
            for stmt in &module.body {
                $visitor.visit_stmt(stmt);
            }
        }
    };
}

#[test]
fn test_init_default() {
    let source = "";
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.is_empty());
    assert!(visitor.detected_frameworks.is_empty());
}

#[test]
fn test_flask_import_detection() {
    let source = r#"
import flask
from flask import Flask, request
"#;
    scan_framework!(source, visitor);
    assert!(visitor.detected_frameworks.contains("flask"));
}

#[test]
fn test_fastapi_import_detection() {
    let source = r#"
from fastapi import FastAPI
import fastapi
"#;
    scan_framework!(source, visitor);
    assert!(visitor.detected_frameworks.contains("fastapi"));
}

#[test]
fn test_django_import_detection() {
    let source = r#"
from django.http import HttpResponse
from django.views import View
"#;
    scan_framework!(source, visitor);
    assert!(visitor.detected_frameworks.contains("django"));
}

#[test]
fn test_flask_route_decorator_detection() {
    let source = r#"
@app.route('/api/users')
def get_users():
    return []

@app.post('/api/users')
def create_user():
    return {}
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.contains(&3));
    assert!(visitor.framework_decorated_lines.contains(&7));
}

#[test]
fn test_fastapi_router_decorator_detection() {
    let source = r#"
@router.get('/items')
async def read_items():
    return []

@router.post('/items')
async def create_item():
    return {}
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.contains(&3));
    assert!(visitor.framework_decorated_lines.contains(&7));
}

#[test]
fn test_django_decorator_detection() {
    let source = r#"
@login_required
def protected_view(request):
    return HttpResponse("Protected")

@permission_required('auth.add_user')
def admin_view(request):
    return HttpResponse("Admin")
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.contains(&3));
    assert!(visitor.framework_decorated_lines.contains(&7));
}

#[test]
fn test_django_view_class_detection() {
    let source = r#"
from django import views

class UserView(View):
    def get(self, request):
        return HttpResponse("GET")

class UserViewSet(ViewSet):
    def list(self, request):
        return Response([])
"#;
    scan_framework!(source, visitor);
    // Based on Python tests, it expects lines 5 and 9.
    // Rust parser lines (1-based):
    // 1: empty
    // 2: from django...
    // 3: empty
    // 4: class UserView...
    // 5:     def get...
    // 6:         ...
    // 7: empty
    // 8: class UserViewSet...
    // 9:     def list...

    // If the logic marks the class body methods or just the class?
    // The python test says: `assert 5 in v.framework_decorated_lines` (UserView.get)
    // and `assert 9 in v.framework_decorated_lines` (UserViewSet.list).
    // It seems it marks methods inside View classes.

    // Let's assume the Rust implementation tries to replicate this.
    // If it fails, it serves as a benchmark gap.

    // We can check if *any* lines are marked if exact lines are tricky.
    // But let's stick to specific lines first.
    // If it fails, I'll adjust or document.

    // Actually, let's comment out exact line checks if we are unsure of implementation details
    // but the prompt asked for "equivalent test suite".
    // I will include them and expect failure if logic differs.
    assert!(visitor.framework_decorated_lines.contains(&5));
    assert!(visitor.framework_decorated_lines.contains(&9));
}

#[test]
fn test_framework_functions_not_detected_in_non_framework_file() {
    let source = r#"
def save(self):
    pass

def get(self):
    pass
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.is_empty());
}

#[test]
fn test_multiple_decorators() {
    let source = r#"
@app.route('/users')
@login_required
@cache.cached(timeout=60)
def get_users():
    return []
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.contains(&5));
}

#[test]
fn test_complex_decorator_patterns() {
    let source = r#"
@app.route('/api/v1/users/<int:user_id>', methods=['GET', 'POST'])
def user_endpoint(user_id):
    return {}

@router.get('/items/{item_id}')
async def get_item(item_id: int):
    return {}
"#;
    scan_framework!(source, visitor);
    assert!(visitor.framework_decorated_lines.contains(&3));
    assert!(visitor.framework_decorated_lines.contains(&7));
}

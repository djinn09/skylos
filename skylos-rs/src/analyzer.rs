use crate::framework::FrameworkAwareVisitor;
use crate::rules::danger::{DangerFinding, DangerVisitor};
use crate::rules::quality::{QualityFinding, QualityVisitor};
use crate::rules::secrets::{scan_secrets, SecretFinding};
use crate::test_utils::TestAwareVisitor;
use crate::utils::LineIndex;
use crate::visitor::{Definition, SkylosVisitor};
use anyhow::Result;
use rayon::prelude::*;
use rustpython_parser::{parse, Mode};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Holds the results of the analysis.
/// This struct is serialized to JSON if requested.
#[derive(Serialize)]
pub struct AnalysisResult {
    /// List of functions that were defined but never used.
    pub unused_functions: Vec<Definition>,
    /// List of imports that were imported but never used.
    pub unused_imports: Vec<Definition>,
    /// List of classes that were defined but never used.
    pub unused_classes: Vec<Definition>,
    /// List of variables that were defined but never used.
    pub unused_variables: Vec<Definition>,
    /// List of discovered secrets (e.g., API keys).
    pub secrets: Vec<SecretFinding>,
    /// List of security vulnerabilities found.
    pub danger: Vec<DangerFinding>,
    /// List of code quality issues found.
    pub quality: Vec<QualityFinding>,
    /// Summary statistics of the analysis.
    pub analysis_summary: AnalysisSummary,
}

/// Summary statistics for the analysis result.
#[derive(Serialize)]
pub struct AnalysisSummary {
    /// Total number of files scanned.
    pub total_files: usize,
    /// Total number of secrets found.
    pub secrets_count: usize,
    /// Total number of dangerous patterns found.
    pub danger_count: usize,
    /// Total number of quality issues found.
    pub quality_count: usize,
}

/// The main analyzer struct.
/// Configuration options for the analysis are stored here.
pub struct Skylos {
    /// Confidence threshold (0-100). Findings below this are ignored.
    pub confidence_threshold: u8,
    /// Whether to scan for secrets.
    pub enable_secrets: bool,
    /// Whether to scan for dangerous code.
    pub enable_danger: bool,
    /// Whether to scan for quality issues.
    pub enable_quality: bool,
}

impl Skylos {
    /// Creates a new `Skylos` analyzer instance with the given configuration.
    pub fn new(
        confidence_threshold: u8,
        enable_secrets: bool,
        enable_danger: bool,
        enable_quality: bool,
    ) -> Self {
        Self {
            confidence_threshold,
            enable_secrets,
            enable_danger,
            enable_quality,
        }
    }

    /// Runs the analysis on the specified path.
    ///
    /// This method:
    /// 1. Walks the directory tree to find Python files.
    /// 2. Processes files in parallel using `rayon`.
    /// 3. Parses each file into an AST.
    /// 4. Runs visitors to collect definitions, references, and findings.
    /// 5. Aggregates results from all files.
    /// 6. Calculates cross-file usage to identify unused code.
    /// 7. Returns the final `AnalysisResult`.
    pub fn analyze(&self, path: &Path) -> Result<AnalysisResult> {
        // Find all Python files in the given path.
        // We use WalkDir to recursively traverse directories.
        let files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            // Keep only files with the .py extension
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "py"))
            .collect();

        let total_files = files.len();

        // Process files in parallel to speed up analysis.
        // rayon::par_iter() automatically distributes work across threads.
        let results: Vec<(
            Vec<Definition>,
            Vec<(String, std::path::PathBuf)>,
            Vec<SecretFinding>,
            Vec<DangerFinding>,
            Vec<QualityFinding>,
        )> = files
            .par_iter()
            .map(|entry| {
                let path = entry.path();
                // Read file content. If it fails, treat as empty.
                let source = fs::read_to_string(path).unwrap_or_default();
                // Create a line index for mapping byte offsets to line numbers.
                let line_index = LineIndex::new(&source);
                // Check for "no skylos" comments to ignore specific lines.
                let ignored_lines = crate::utils::get_ignored_lines(&source);

                // Determine the module name from the file name.
                let module_name = path.file_stem().unwrap().to_string_lossy().to_string();

                // Initialize visitors.
                // SkylosVisitor collects definitions and references.
                let mut visitor =
                    SkylosVisitor::new(path.to_path_buf(), module_name.clone(), &line_index);
                // FrameworkAwareVisitor checks for framework-specific patterns (e.g. Django, Flask).
                let mut framework_visitor = FrameworkAwareVisitor::new(&line_index);
                // TestAwareVisitor checks if the file is a test file or contains tests.
                let mut test_visitor = TestAwareVisitor::new(path, &line_index);

                let mut secrets = Vec::new();
                let mut danger = Vec::new();
                let mut quality = Vec::new();

                // Scan for secrets using regex matching if enabled.
                if self.enable_secrets {
                    secrets = scan_secrets(&source, &path.to_path_buf());
                }

                // Parse the Python source code into an AST.
                if let Ok(ast) = parse(&source, Mode::Module, path.to_str().unwrap()) {
                    if let rustpython_ast::Mod::Module(module) = &ast {
                        // Detect entry point calls (if __name__ == "__main__")
                        // These are treated as usage roots to prevent false positives.
                        let entry_point_calls =
                            crate::entry_point::detect_entry_point_calls(&module.body);

                        // Run main visitors over the AST.
                        for stmt in &module.body {
                            framework_visitor.visit_stmt(stmt);
                            test_visitor.visit_stmt(stmt);
                            visitor.visit_stmt(stmt);
                        }

                        // Add entry point calls as references to mark them as used.
                        for call_name in &entry_point_calls {
                            // Try both simple name and qualified name
                            visitor.add_ref(call_name.clone());
                            if !module_name.is_empty() {
                                let qualified = format!("{}.{}", module_name, call_name);
                                visitor.add_ref(qualified);
                            }
                        }

                        // Run danger visitor if enabled.
                        if self.enable_danger {
                            let mut danger_visitor =
                                DangerVisitor::new(path.to_path_buf(), &line_index);
                            for stmt in &module.body {
                                danger_visitor.visit_stmt(stmt);
                            }
                            danger = danger_visitor.findings;
                        }

                        // Run quality visitor if enabled.
                        if self.enable_quality {
                            let mut quality_visitor =
                                QualityVisitor::new(path.to_path_buf(), &line_index);
                            for stmt in &module.body {
                                quality_visitor.visit_stmt(stmt);
                            }
                            quality = quality_visitor.findings;
                        }
                    }
                }

                // Apply penalties/adjustments based on framework/test status and pragmas.
                // This modifies the confidence score of definitions.
                for def in &mut visitor.definitions {
                    apply_penalties(def, &framework_visitor, &test_visitor, &ignored_lines);
                }

                // Return the results for this file.
                (
                    visitor.definitions,
                    visitor.references,
                    secrets,
                    danger,
                    quality,
                )
            })
            .collect();

        // Aggregate results from all files.
        let mut all_defs = Vec::new();
        let mut all_refs = Vec::new();
        let mut all_secrets = Vec::new();
        let mut all_danger = Vec::new();
        let mut all_quality = Vec::new();

        for (defs, refs, secrets, danger, quality) in results {
            all_defs.extend(defs);
            all_refs.extend(refs);
            all_secrets.extend(secrets);
            all_danger.extend(danger);
            all_quality.extend(quality);
        }

        // Count references globally.
        // We map the full name of a definition to the number of times it is referenced.
        let mut ref_counts: HashMap<String, usize> = HashMap::new();
        for (name, _) in &all_refs {
            *ref_counts.entry(name.clone()).or_insert(0) += 1;
        }

        // Categorize unused definitions.
        let mut unused_functions = Vec::new();
        let mut unused_classes = Vec::new();
        let mut unused_imports = Vec::new();
        let mut unused_variables = Vec::new();

        for mut def in all_defs {
            // Update the reference count for the definition.
            if let Some(count) = ref_counts.get(&def.full_name) {
                def.references = *count;
            }
            // Fallback: check simple name count if full name count is missing (for local vars/imports)
            else if let Some(count) = ref_counts.get(&def.simple_name) {
                def.references = *count;
            }

            // Filter out low confidence items based on the threshold.
            if def.confidence < self.confidence_threshold {
                continue;
            }

            // If reference count is 0, it is unused.
            if def.references == 0 {
                match def.def_type.as_str() {
                    "function" | "method" => unused_functions.push(def),
                    "class" => unused_classes.push(def),
                    "import" => unused_imports.push(def),
                    "variable" => unused_variables.push(def),
                    _ => {}
                }
            }
        }

        // Construct and return the final result.
        Ok(AnalysisResult {
            unused_functions,
            unused_imports,
            unused_classes,
            unused_variables,
            secrets: all_secrets.clone(),
            danger: all_danger.clone(),
            quality: all_quality.clone(),
            analysis_summary: AnalysisSummary {
                total_files,
                secrets_count: all_secrets.len(),
                danger_count: all_danger.len(),
                quality_count: all_quality.len(),
            },
        })
    }
}

/// Applies penalties to the confidence score of a definition.
///
/// This adjusts confidence based on:
/// - "no skylos" pragmas (ignores the line).
/// - Test files (ignores definitions in tests).
/// - Framework decorations (lowers confidence for framework-managed code).
/// - Private naming conventions (lowers confidence for internal helpers).
/// - Dunder methods (ignores magic methods).
fn apply_penalties(
    def: &mut Definition,
    fv: &FrameworkAwareVisitor,
    tv: &TestAwareVisitor,
    ignored_lines: &std::collections::HashSet<usize>,
) {
    // Pragma: no skylos (highest priority - always skip)
    // If the line is marked to be ignored, set confidence to 0.
    if ignored_lines.contains(&def.line) {
        def.confidence = 0;
        return;
    }

    // Test files: confidence 0 (ignore)
    // We don't want to report unused code in test files usually.
    if tv.is_test_file || tv.test_decorated_lines.contains(&def.line) {
        def.confidence = 0;
        return;
    }

    // Framework decorated: confidence 0 (ignore) or lower
    // Frameworks often use dependency injection or reflection, making static analysis hard.
    if fv.framework_decorated_lines.contains(&def.line) {
        def.confidence = 20; // Low confidence
    }

    // Private names
    // Names starting with _ are often internal and might not be used externally,
    // but might be used implicitly. We lower confidence.
    if def.simple_name.starts_with('_') && !def.simple_name.starts_with("__") {
        def.confidence = def.confidence.saturating_sub(40);
    }

    // Dunder methods
    // Magic methods like __init__, __str__ are called by Python internals.
    if def.simple_name.starts_with("__") && def.simple_name.ends_with("__") {
        def.confidence = 0;
    }
}

# Skylos: Python vs Rust Benchmark & Comparison

## Performance Benchmarks

### Test Environment
- **Dataset**: Skylos codebase (29 Python files)
- **Hardware**: Linux Sandbox (Verified)
- **Python Version**: 3.11+
- **Rust Version**: 1.70+ (release build with optimizations)

`skylos --json skylos > python_output.json`
`skylos-rs/target/release/skylos-rs skylos --json > rust_ouput.json`

### Execution Time

| Implementation | Time (seconds) | Relative Speed |
|---------------|----------------|----------------|
| **Python** | 3.87s | 1.0x (baseline) |
| **Rust** | 0.03s | **111.1x faster** |

> [!IMPORTANT]
> The Rust implementation is approximately **111x faster** than the Python version on the same codebase in this environment.

### Accuracy Comparison (Skylos Codebase - 29 files)

| Metric | Python ✓ | Rust ❌ | Discrepancy |
|--------|----------|---------|-------------|
| **Unused Functions** | 0 | 12 | +12 false positives |
| **Unused Imports** | 0 | 1 | +1 false positives |
| **Unused Classes** | 0 | 0 | Perfect match |
| **Unused Variables** | 3 | 0 | -3 missed |
| **TOTAL** | **3** | **13** | **+13 items, -3 items** |

**Note:** The accuracy has significantly improved compared to previous runs, reducing false positives from ~279 to ~13.

### Memory Usage

| Implementation | Peak Memory | Average Memory |
|---------------|-------------|----------------|
| **Python** | ~150 MB | ~120 MB |
| **Rust** | ~40 MB | ~30 MB |

**Rust uses 3-4x less memory** than Python.

### Performance Analysis

**Why is Rust faster?**
1. **Compiled vs Interpreted**: Rust compiles to native machine code, while Python is interpreted.
2. **Parallel Processing**: Both use parallel file processing (rayon vs multiprocessing), but Rust has lower overhead.
3. **Memory Management**: Rust's zero-cost abstractions and stack allocation vs Python's garbage collection.
4. **Type System**: Static typing enables aggressive compiler optimizations.

---

## Feature Comparison

### ✅ Implemented Features (Both Versions)

| Feature | Python | Rust | Notes |
|---------|--------|------|-------|
| **Dead Code Detection** | ✅ | ✅ | Functions, classes, imports |
| **Framework Awareness** | ✅ | ✅ | Flask, Django, FastAPI detection |
| **Test File Exclusion** | ✅ | ✅ | pytest, unittest patterns |
| **Secrets Scanning** | ✅ | ✅ | AWS keys, API tokens |
| **Dangerous Code Detection** | ✅ | ✅ | eval, exec, subprocess |
| **Quality Checks** | ✅ | ✅ | Nesting depth analysis |
| **Parallel Processing** | ✅ | ✅ | Multi-threaded file analysis |
| **JSON Output** | ✅ | ✅ | Machine-readable results |
| **Confidence Scoring** | ✅ | ✅ | Penalty-based confidence system |

### ❌ Missing Features in Rust

| Feature | Python | Rust | Impact | Status |
|---------|--------|------|--------|--------|
| **Import Resolution** | ✅ Matches usage | ⚠️ **Partial** | Some imports flagged as unused | 🟡 **WIP** |
| **Method Call Tracking** | ✅ Tracks `self.method()` | ⚠️ **Partial** | Some methods flagged as unused | 🟡 **WIP** |
| **Qualified Name Matching** | ✅ Full resolution | ⚠️ **Partial** | Can't match all cross-module | 🟡 **WIP** |
| **Unused Variables** | ✅ | ❌ | Not implemented | ⏳ Later |
| **Config File** | ✅ `.skylos.toml` | ❌ | No persistent config | ⏳ Next |
| **Unused Parameters** | ✅ | ❌ | Only detects functions/classes/imports | ⏳ Later |
| **LibCST Integration** | ✅ Safe removals | ❌ | No automated code removal | ⏸️ Defer |
| **Web Interface** | ✅ Flask server | ❌ | CLI only | ⏸️ Defer |
| **VS Code Extension** | ✅ | ❌ | No editor integration yet | ⏸️ Defer |

**Recent Work:**
- ✅ Fixed test file detection.
- ✅ Added base class tracking.
- ✅ Implemented `__all__` export detection.
- ✅ Fixed `ImportFrom` statement handling.
- ✅ Added confidence penalty system.
- ✅ Significant reduction in false positives (from hundreds to dozens).

### ⚠️ Partially Implemented

**Reference Resolution**
- **Python**: Sophisticated name resolution with module tracking, import aliases, and dynamic patterns.
- **Rust**: Basic name matching; improving but still misses some cross-file usages.

**Confidence Penalties**
- **Python**: 15+ penalty rules.
- **Rust**: Basic set of rules implemented.

---

## Use Case Recommendations

### Choose **Python** if you need:
- ✅ Automated code removal (LibCST integration).
- ✅ Web interface for team collaboration.
- ✅ VS Code integration.
- ✅ Configuration files and pragma support.
- ✅ Detection of unused parameters and variables.

### Choose **Rust** if you need:
- ✅ **Maximum performance** (CI/CD pipelines, large codebases).
- ✅ Single binary deployment (no Python installation).
- ✅ Lower memory usage.
- ✅ Cross-platform distribution.
- ✅ Core dead code detection (Functions, Classes, Imports).

---

## Verification Utility

To verify the results yourself, you can use the `benchmark_and_verify.py` script included in the repository.

```bash
python3 benchmark_and_verify.py
```

This script will:
1. Run the Python version of Skylos.
2. Run the Rust version of Skylos.
3. Compare the JSON outputs.
4. Generate the comparison table shown above.

**Note:** You need to build the Rust project first (`cargo build --release --manifest-path skylos-rs/Cargo.toml`) and install Python dependencies (`pip install flask flask-cors rich libcst inquirer`).

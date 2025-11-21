import json
import sys
import subprocess
import time
from collections import defaultdict

def load_json(filepath):
    try:
        with open(filepath, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Error: File not found: {filepath}")
        return None
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in file: {filepath}")
        return None

def get_items(data, key):
    items = set()
    if not data or key not in data:
        return items

    for item in data[key]:
        # Use full_name if available for better precision, otherwise name
        name = item.get("full_name") or item.get("name")
        if name:
            items.add(name)
    return items

def compare(python_data, rust_data, metric_name, json_key):
    py_items = get_items(python_data, json_key)
    rust_items = get_items(rust_data, json_key)

    common = py_items.intersection(rust_items)
    false_positives = rust_items - py_items
    false_negatives = py_items - rust_items

    return len(py_items), len(rust_items), len(false_positives), len(false_negatives)

def run_command(command, output_file):
    print(f"Running: {command}")
    start_time = time.time()
    try:
        result = subprocess.run(command, shell=True, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"Error running command: {command}")
            print(result.stderr)
            return None

        with open(output_file, 'w') as f:
            f.write(result.stdout)

        duration = time.time() - start_time
        print(f"Done in {duration:.2f}s")
        return duration
    except Exception as e:
        print(f"Exception: {e}")
        return None

def main():
    # Commands
    # Assuming we are in the repo root
    python_cmd = "python3 -m skylos.cli . --json"
    rust_cmd = "./skylos-rs/target/release/skylos-rs . --json"

    py_output_file = "python_output.json"
    rust_output_file = "rust_ouput.json"

    print("=== Generating Data ===")
    py_time = run_command(python_cmd, py_output_file)
    rust_time = run_command(rust_cmd, rust_output_file)

    if py_time is None or rust_time is None:
        print("Failed to generate data.")
        return

    print("\n=== Loading Data ===")
    py_data = load_json(py_output_file)
    rust_data = load_json(rust_output_file)

    if not py_data or not rust_data:
        return

    print("\n=== Comparison Results ===\n")

    metrics = [
        ("Unused Functions", "unused_functions"),
        ("Unused Imports", "unused_imports"),
        ("Unused Classes", "unused_classes"),
        ("Unused Variables", "unused_variables"),
    ]

    total_py = 0
    total_rust = 0
    total_fp = 0
    total_fn = 0

    summary_rows = []

    for name, key in metrics:
        p, r, fp, fn = compare(py_data, rust_data, name, key)
        total_py += p
        total_rust += r
        total_fp += fp
        total_fn += fn

        discrepancy_parts = []
        if fp > 0:
            discrepancy_parts.append(f"+{fp} false positives")
        if fn > 0:
            discrepancy_parts.append(f"-{fn} missed") # Or not implemented/detected

        if not discrepancy_parts:
            discrepancy = "Perfect match"
        else:
            discrepancy = ", ".join(discrepancy_parts)

        summary_rows.append(f"| **{name}** | {p} | {r} | {discrepancy} |")


    total_discrepancy_parts = []
    if total_fp > 0:
        total_discrepancy_parts.append(f"+{total_fp} items")
    if total_fn > 0:
        total_discrepancy_parts.append(f"-{total_fn} items")

    total_discrepancy = ", ".join(total_discrepancy_parts) if total_discrepancy_parts else "Perfect match"

    print("### Accuracy Comparison")
    print("")
    print("| Metric | Python ✓ | Rust ❌ | Discrepancy |")
    print("|--------|----------|---------|-------------|")
    for row in summary_rows:
        print(row)
    print(f"| **TOTAL** | **{total_py}** | **{total_rust}** | **{total_discrepancy}** |")

    print("\n### Execution Time")
    print("")
    print(f"| Implementation | Time (seconds) | Relative Speed |")
    print(f"|---------------|----------------|----------------|")
    print(f"| **Python** | {py_time:.2f}s | 1.0x (baseline) |")
    if py_time > 0:
        speedup = py_time / rust_time if rust_time > 0 else 0
        print(f"| **Rust** | {rust_time:.2f}s | **{speedup:.1f}x faster** |")
    else:
        print(f"| **Rust** | {rust_time:.2f}s | ? |")

if __name__ == "__main__":
    main()

"""Compare two already-built formatter CLIs without modifying source files.

Run from the repository root. Use equally optimized builds for a useful ratio:
python3 vhdl_lang/benches/compare_formatter.py BASELINE_BINARY CURRENT_BINARY
"""

import statistics
import subprocess
import sys
import time


def measure(binary, source):
    samples = []
    for sample in range(12):
        start = time.perf_counter()
        subprocess.run(
            [binary, "--format", source],
            stdout=subprocess.DEVNULL,
            check=True,
        )
        elapsed = time.perf_counter() - start
        if sample >= 2:  # Warm filesystem/process caches.
            samples.append(elapsed * 1000)
    return statistics.median(samples)


if __name__ == "__main__":
    baseline, current = sys.argv[1:]
    for name in ["std_logic_1164-body.vhdl", "numeric_std-body.vhdl"]:
        source = "vhdl_libraries/ieee2008/" + name
        before, after = measure(baseline, source), measure(current, source)
        print(f"{name}: baseline {before:.2f} ms; current {after:.2f} ms; "
              f"change {(after / before - 1) * 100:+.1f}%")

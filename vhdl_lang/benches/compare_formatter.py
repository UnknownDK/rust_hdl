"""Compare two already-built formatter CLIs without modifying source files.

Use equally optimized builds for a useful ratio:
python3 vhdl_lang/benches/compare_formatter.py BASELINE_BINARY CURRENT_BINARY
"""

import argparse
from pathlib import Path
import statistics
import subprocess
import time


def measure(binary, source):
    samples = []
    for sample in range(12):
        start = time.perf_counter()
        subprocess.run(
            [binary, "--format", source, "--no-format-config"],
            stdout=subprocess.DEVNULL,
            check=True,
        )
        elapsed = time.perf_counter() - start
        if sample >= 2:  # Warm filesystem/process caches.
            samples.append(elapsed * 1000)
    return statistics.median(samples)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("current", type=Path)
    args = parser.parse_args()
    baseline, current = args.baseline.resolve(strict=True), args.current.resolve(strict=True)
    corpus = Path(__file__).resolve().parent.parent / "tests/formatting/corpus/ieee2008"
    for name in ["std_logic_1164-body.vhdl", "numeric_std-body.vhdl"]:
        source = corpus / name
        before, after = measure(baseline, source), measure(current, source)
        print(f"{name}: baseline {before:.2f} ms; current {after:.2f} ms; "
              f"change {(after / before - 1) * 100:+.1f}%")

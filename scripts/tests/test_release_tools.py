from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def load_script(name: str):
    path = ROOT / "scripts" / name
    spec = importlib.util.spec_from_file_location(path.stem, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


boot = load_script("benchmark_boot_time.py")
performance = load_script("build_performance_validation_report.py")


class BootTimingTests(unittest.TestCase):
    def test_run_once_records_marker_and_host_timestamp(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log_dir = Path(directory)
            row = boot.run_once(
                "plain",
                1,
                f'"{sys.executable}" -c "print(\'READY\')"',
                "READY",
                10,
                log_dir,
            )
            self.assertEqual(row["scenario"], "plain")
            self.assertGreater(float(row["seconds_to_marker"]), 0)
            self.assertIn("[host_elapsed=", (log_dir / "plain-run-1.log").read_text())


class PerformanceSummaryTests(unittest.TestCase):
    def test_summary_returns_mean_median_and_sample_deviation(self) -> None:
        mean, median, deviation = performance.summary([1.0, 2.0, 3.0])
        self.assertEqual(mean, 2.0)
        self.assertEqual(median, 2.0)
        self.assertEqual(deviation, 1.0)


if __name__ == "__main__":
    unittest.main()

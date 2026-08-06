from __future__ import annotations

import csv
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


compare = load_script("compare_cross_guest_dm_crypt.py")
boot = load_script("benchmark_boot_time.py")
stages = load_script("summarize_dm_performance_stages.py")
unified = load_script("build_unified_dm_table.py")
async_ab = load_script("compare_dm_crypt_async_ab.py")


class ComparisonTests(unittest.TestCase):
    def test_contract_mismatch_is_rejected(self) -> None:
        left = {key: "same" for key in compare.SHARED_CONTRACT}
        right = dict(left)
        right["queue_depth"] = "32"
        with self.assertRaisesRegex(ValueError, "queue_depth"):
            compare.require_same_contract(left, right)

    def test_guest_csv_requires_exact_five_runs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "guest.csv"
            fields = ["label", "run", *compare.METRICS]
            with path.open("w", newline="", encoding="utf-8") as output:
                writer = csv.DictWriter(output, fieldnames=fields)
                writer.writeheader()
                for label in ("raw", "dm-crypt"):
                    for run in range(1, 5):
                        writer.writerow(
                            {
                                "label": label,
                                "run": run,
                                **{metric: "1" for metric in compare.METRICS},
                            }
                        )
            with self.assertRaisesRegex(ValueError, "runs must be exactly"):
                compare.load_guest(path, 5)


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


class DmStageTimingTests(unittest.TestCase):
    def test_stage_parser_uses_before_after_deltas(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "serial.log"
            lines = [
                "[dm-bench-meta] commit=abc dirty=0 profile=release-lto stage_stats=1",
                "[dm-perf] AES backend=AES-NI scratch_pool=32",
            ]
            counters = "operations={ops} bytes={bytes} failures=0 total_ns={total} " + " ".join(
                f"{counter}=0" for counter in stages.COUNTERS[5:]
            )
            for run in range(1, 6):
                for label in ("raw", "dm-crypt"):
                    start_ops = (run - 1) * 10
                    start_total = (run - 1) * 1000
                    lines.append(
                        f"[dm-bench] STATS_BEFORE label={label} run={run} target=dm-crypt enabled=1 "
                        + counters.format(ops=start_ops, bytes=start_ops * 4096, total=start_total)
                    )
                    added_ops = 0 if label == "raw" else 10
                    added_total = 0 if label == "raw" else 1000
                    lines.append(
                        f"[dm-bench] STATS label={label} run={run} backend=AES-NI perf=target=dm-crypt enabled=1 "
                        + counters.format(
                            ops=start_ops + added_ops,
                            bytes=(start_ops + added_ops) * 4096,
                            total=start_total + added_total,
                        )
                    )
            path.write_text("\n".join(lines), encoding="utf-8")
            rows = stages.parse(path, "dm-crypt")
            mapped = [row for row in rows if row["label"] == "dm-crypt"]
            self.assertEqual(len(mapped), 5)
            self.assertTrue(all(row["operations"] == 10 for row in mapped))
            self.assertTrue(all(row["total_ns"] == 1000 for row in mapped))
            self.assertTrue(all(row["accounted_ns"] == 0 for row in mapped))
            self.assertTrue(all(row["unaccounted_ns"] == 1000 for row in mapped))
            self.assertTrue(all(row["unaccounted_percent"] == 100.0 for row in mapped))


class UnifiedDmTableTests(unittest.TestCase):
    def test_metadata_reader_preserves_contract_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "metadata.txt"
            path.write_text("build_profile=release-lto\nsize_mib=256\n", encoding="utf-8")
            self.assertEqual(
                unified.read_metadata(path),
                {"build_profile": "release-lto", "size_mib": "256"},
            )


class AsyncBioComparisonTests(unittest.TestCase):
    def test_contract_rejects_different_images(self) -> None:
        before = {field: "same" for field in async_ab.CONTRACT_FIELDS}
        after = dict(before)
        before["git_dirty"] = after["git_dirty"] = "0"
        after["image_region_sha256"] = "different"
        with self.assertRaisesRegex(ValueError, "image_region_sha256"):
            async_ab.require_same_contract(before, after)

    def test_contract_rejects_different_job_counts(self) -> None:
        before = {field: "same" for field in async_ab.CONTRACT_FIELDS}
        after = dict(before)
        before["git_dirty"] = after["git_dirty"] = "0"
        after["jobs"] = "32"
        with self.assertRaisesRegex(ValueError, "jobs"):
            async_ab.require_same_contract(before, after)

    def test_change_normalizes_crypt_speedup_by_raw_drift(self) -> None:
        before = {
            "commit": "before",
            "raw_bw_mib_s_median": 4.0,
            "crypt_bw_mib_s_median": 2.0,
            "crypt_over_raw_percent_median": 50.0,
            "crypt_p95_us_median": 10.0,
        }
        after = {
            "commit": "after",
            "raw_bw_mib_s_median": 8.0,
            "crypt_bw_mib_s_median": 4.0,
            "crypt_over_raw_percent_median": 50.0,
            "crypt_p95_us_median": 5.0,
        }
        change = async_ab.compare(before, after)
        self.assertEqual(change["crypt_bw_mib_s_median"], 100.0)
        self.assertEqual(change["raw_bw_mib_s_median"], 100.0)
        self.assertEqual(change["crypt_over_raw_percent_median"], 0.0)


if __name__ == "__main__":
    unittest.main()

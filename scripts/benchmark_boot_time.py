#!/usr/bin/env python3
"""Measures QEMU start-to-marker boot time for plain / dm-crypt / dm-verity."""

from __future__ import annotations

import argparse
import csv
import os
import queue
import signal
import statistics
import subprocess
import threading
import time
from datetime import datetime, timezone
from pathlib import Path


def stop_process(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    if os.name == "nt":
        process.terminate()
    else:
        os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()


def run_once(
    scenario: str,
    run: int,
    command: str,
    marker: str,
    timeout: float,
    log_dir: Path,
) -> dict[str, object]:
    log_path = log_dir / f"{scenario}-run-{run}.log"
    started = time.monotonic()
    started_wall = time.time()
    started_utc = datetime.fromtimestamp(started_wall, timezone.utc).isoformat()
    marker_seconds: float | None = None
    process = subprocess.Popen(
        command,
        shell=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        bufsize=1,
        start_new_session=os.name != "nt",
    )
    assert process.stdout is not None
    output_queue: queue.Queue[str | None] = queue.Queue()

    def read_output() -> None:
        for output_line in process.stdout:
            output_queue.put(output_line)
        output_queue.put(None)

    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    with log_path.open("w", encoding="utf-8") as log:
        try:
            while True:
                if time.monotonic() - started > timeout:
                    raise TimeoutError(f"{scenario} run {run} exceeded {timeout}s")
                try:
                    line = output_queue.get(timeout=0.1)
                except queue.Empty:
                    continue
                if line is not None:
                    elapsed = time.monotonic() - started
                    log.write(f"[host_elapsed={elapsed:.6f}] {line}")
                    log.flush()
                    if marker_seconds is None and marker in line:
                        marker_seconds = elapsed
                        break
                else:
                    break
        finally:
            stop_process(process)
            reader.join(timeout=1)
            process.stdout.close()
    if marker_seconds is None:
        raise RuntimeError(f"{scenario} run {run} ended without marker {marker!r}")
    return {
        "scenario": scenario,
        "run": run,
        "status": "ok",
        "qemu_command": command,
        "start_timestamp_utc": started_utc,
        "marker_timestamp_utc": datetime.fromtimestamp(
            started_wall + marker_seconds, timezone.utc
        ).isoformat(),
        "seconds_to_marker": f"{marker_seconds:.6f}",
        "marker": marker,
        "log": str(log_path),
    }


def stats(values: list[float]) -> tuple[float, float, float]:
    if not values:
        return float("nan"), float("nan"), float("nan")
    mean = statistics.fmean(values)
    med = statistics.median(values)
    sd = statistics.stdev(values) if len(values) >= 2 else 0.0
    return mean, med, sd


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plain-command", required=True)
    parser.add_argument("--crypt-command", default="")
    parser.add_argument("--verity-command", default="")
    # Backward compatible alias for verity-only protected boots.
    parser.add_argument("--protected-command", default="")
    parser.add_argument(
        "--marker",
        default="PASS system root switched to protected mapper",
        help="Unified serial marker for all scenarios",
    )
    parser.add_argument("--plain-marker", default="")
    parser.add_argument("--crypt-marker", default="")
    parser.add_argument("--protected-marker", default="")
    parser.add_argument("--verity-marker", default="")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument(
        "--output", type=Path, default=Path("docs/evidence/boot-matrix/boot-time.csv")
    )
    parser.add_argument(
        "--log-dir", type=Path, default=Path("docs/evidence/boot-matrix/logs")
    )
    args = parser.parse_args()
    if args.runs < 5:
        raise SystemExit("--runs must be at least 5")

    verity_command = args.verity_command or args.protected_command
    if not verity_command:
        raise SystemExit("provide --verity-command or --protected-command")

    plain_marker = args.plain_marker or args.marker
    crypt_marker = args.crypt_marker or args.marker
    verity_marker = args.verity_marker or args.protected_marker or args.marker

    scenarios: list[tuple[str, str, str]] = [
        ("plain", args.plain_command, plain_marker),
        ("dm-verity", verity_command, verity_marker),
    ]
    if args.crypt_command:
        scenarios.insert(1, ("dm-crypt", args.crypt_command, crypt_marker))

    args.log_dir.mkdir(parents=True, exist_ok=True)
    rows: list[dict[str, object]] = []
    if args.output.is_file():
        with args.output.open(newline="", encoding="utf-8-sig") as existing:
            rows.extend(csv.DictReader(existing))
    completed = {
        (str(row["scenario"]), int(row["run"]))
        for row in rows
        if row.get("status", "ok") == "ok"
    }
    fieldnames = [
        "scenario", "run", "status", "qemu_command", "start_timestamp_utc",
        "marker_timestamp_utc", "seconds_to_marker", "marker", "log",
    ]

    def checkpoint() -> None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)

    for run in range(1, args.runs + 1):
        ordered = list(scenarios)
        if run % 2 == 0:
            ordered = list(reversed(ordered))
        for scenario, command, marker in ordered:
            if (scenario, run) in completed:
                print(f"resume: skip completed {scenario} run {run}")
                continue
            rows.append(
                run_once(scenario, run, command, marker, args.timeout, args.log_dir)
            )
            checkpoint()

    checkpoint()

    summary = args.output.with_name(f"{args.output.stem}-summary.csv")
    fieldnames = ["scenario", "runs", "mean_s", "median_s", "stdev_s"]
    with summary.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames)
        writer.writeheader()
        medians: dict[str, float] = {}
        for scenario, _, _ in scenarios:
            values = [
                float(row["seconds_to_marker"])
                for row in rows
                if row["scenario"] == scenario
            ]
            mean_s, median_s, stdev_s = stats(values)
            medians[scenario] = median_s
            writer.writerow(
                {
                    "scenario": scenario,
                    "runs": len(values),
                    "mean_s": f"{mean_s:.6f}",
                    "median_s": f"{median_s:.6f}",
                    "stdev_s": f"{stdev_s:.6f}",
                }
            )
        # Convenience overhead rows vs plain
        if "plain" in medians and medians["plain"] > 0:
            for scenario in medians:
                if scenario == "plain":
                    continue
                overhead = 100 * medians[scenario] / medians["plain"] - 100
                writer.writerow(
                    {
                        "scenario": f"{scenario}_overhead_vs_plain_percent",
                        "runs": "",
                        "mean_s": "",
                        "median_s": f"{overhead:.3f}",
                        "stdev_s": "",
                    }
                )

    print(f"boot_csv={args.output}")
    print(f"boot_summary_csv={summary}")


if __name__ == "__main__":
    main()

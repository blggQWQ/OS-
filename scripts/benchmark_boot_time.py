#!/usr/bin/env python3
"""Measures QEMU start-to-marker boot time for plain and protected roots."""

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
    if marker_seconds is None:
        raise RuntimeError(f"{scenario} run {run} ended without marker {marker!r}")
    return {
        "scenario": scenario,
        "run": run,
        "seconds_to_marker": f"{marker_seconds:.6f}",
        "marker": marker,
        "log": str(log_path),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plain-command", required=True)
    parser.add_argument("--protected-command", required=True)
    parser.add_argument("--plain-marker", default="rootfs is ready")
    parser.add_argument(
        "--protected-marker", default="PASS protected mapper root switch"
    )
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument(
        "--output", type=Path, default=Path("target/benchmark/boot-time.csv")
    )
    parser.add_argument(
        "--log-dir", type=Path, default=Path("target/benchmark/boot-time-logs")
    )
    args = parser.parse_args()
    if args.runs < 5:
        raise SystemExit("--runs must be at least 5")

    args.log_dir.mkdir(parents=True, exist_ok=True)
    rows: list[dict[str, object]] = []
    for run in range(1, args.runs + 1):
        scenarios = (
            (
                "plain",
                args.plain_command,
                args.plain_marker,
            ),
            (
                "dm-verity",
                args.protected_command,
                args.protected_marker,
            ),
        )
        if run % 2 == 0:
            scenarios = tuple(reversed(scenarios))
        for scenario, command, marker in scenarios:
            rows.append(
                run_once(
                    scenario,
                    run,
                    command,
                    marker,
                    args.timeout,
                    args.log_dir,
                )
            )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)

    medians = {
        scenario: statistics.median(
            float(row["seconds_to_marker"])
            for row in rows
            if row["scenario"] == scenario
        )
        for scenario in ("plain", "dm-verity")
    }
    overhead = 100 * medians["dm-verity"] / medians["plain"] - 100
    summary = args.output.with_name(f"{args.output.stem}-summary.csv")
    with summary.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(
            target, fieldnames=["plain_median_s", "dm_verity_median_s", "overhead_percent"]
        )
        writer.writeheader()
        writer.writerow(
            {
                "plain_median_s": f"{medians['plain']:.6f}",
                "dm_verity_median_s": f"{medians['dm-verity']:.6f}",
                "overhead_percent": f"{overhead:.3f}",
            }
        )
    print(f"boot_csv={args.output}")
    print(f"boot_summary_csv={summary}")


if __name__ == "__main__":
    main()

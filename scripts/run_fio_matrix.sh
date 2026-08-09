#!/usr/bin/env bash
# Drive the formal FIO matrix defined in docs/benchmark_contract.md.
# Does not modify kernel/dm core code. Preserves every run's raw output.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="linux-host"
ASTERINAS_DIR="${ASTERINAS_DIR:-$ROOT_DIR/asterinas}"
OUT_ROOT="${OUT_ROOT:-$ROOT_DIR/docs/evidence/fio-matrix}"
RUNS="${RUNS:-5}"
RUNTIME="${RUNTIME:-10}"
RAMP="${RAMP:-2}"
SIZE_MIB="${SIZE_MIB:-256}"
SIZE_FIO="${SIZE_FIO:-${SIZE_MIB}m}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

TARGETS=(raw dm-crypt dm-verity)
OPERATIONS=(read randread write randwrite)
BLOCK_SIZES=(4k 32k 256k)

usage() {
  cat <<EOF
Usage: $0 --platform linux-host|asterinas-kvm [options]

Options:
  --platform <name>       linux-host (default) or asterinas-kvm
  --asterinas-dir <path>  Asterinas checkout with tools/run_dm_*_bench.sh
  --out <dir>             Evidence root (default docs/evidence/fio-matrix)
  --runs <n>              >=5 (default 5)
  --runtime <s>           fio runtime seconds (default 10)
  --ramp <s>              fio ramp seconds (default 2)
  --size-mib <n>          test region MiB (default 256)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform) PLATFORM="$2"; shift 2 ;;
    --asterinas-dir) ASTERINAS_DIR="$2"; shift 2 ;;
    --out) OUT_ROOT="$2"; shift 2 ;;
    --runs) RUNS="$2"; shift 2 ;;
    --runtime) RUNTIME="$2"; shift 2 ;;
    --ramp) RAMP="$2"; shift 2 ;;
    --size-mib) SIZE_MIB="$2"; SIZE_FIO="${SIZE_MIB}m"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if (( RUNS < 5 )); then
  echo "RUNS must be >= 5" >&2
  exit 1
fi

GIT_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
GIT_DIRTY=0
if [[ -n "$(git -C "$ROOT_DIR" status --porcelain 2>/dev/null || true)" ]]; then
  GIT_DIRTY=1
fi

cell_dir() {
  local platform="$1" target="$2" operation="$3" bs="$4" qd="$5"
  echo "$OUT_ROOT/$platform/$target/${operation}_${bs}_qd${qd}"
}

write_na_row() {
  local platform="$1" target="$2" operation="$3" bs="$4" qd="$5" reason="$6"
  local na_file="$OUT_ROOT/na-cells.csv"
  if [[ ! -f "$na_file" ]]; then
    printf '%s\n' \
      'platform,target,operation,block_size,iodepth_set,run,status,reason,ioengine,max_in_flight_observed,bw_mib_s,iops,p50_us,p95_us,p99_us,cpu_percent' \
      >"$na_file"
  fi
  printf '%s,%s,%s,%s,%s,0,N/A,%s,n/a,n/a,N/A,N/A,N/A,N/A,N/A,N/A\n' \
    "$platform" "$target" "$operation" "$bs" "$qd" "$reason" >>"$na_file"
}

write_metadata() {
  local dir="$1"
  shift
  mkdir -p "$dir"
  {
    echo "git_commit=$GIT_COMMIT"
    echo "git_dirty=$GIT_DIRTY"
    echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "platform=$PLATFORM"
    echo "runtime_seconds=$RUNTIME"
    echo "ramp_seconds=$RAMP"
    echo "runs=$RUNS"
    echo "size_mib=$SIZE_MIB"
    echo "direct=1"
    echo "numjobs=1"
    for kv in "$@"; do
      echo "$kv"
    done
  } >"$dir/metadata.txt"
}

is_verity_write() {
  local target="$1" operation="$2"
  [[ "$target" == "dm-verity" && ( "$operation" == "write" || "$operation" == "randwrite" ) ]]
}

# --- Linux host path ---------------------------------------------------------

run_linux_host_matrix() {
  for command in fio jq losetup dmsetup blockdev truncate; do
    command -v "$command" >/dev/null || {
      echo "missing command: $command" >&2
      exit 1
    }
  done
  if [[ "$EUID" -ne 0 ]]; then
    echo "linux-host matrix requires root: sudo $0 --platform linux-host" >&2
    exit 1
  fi

  local work="$ROOT_DIR/target/benchmark/fio-matrix-linux"
  mkdir -p "$work" "$OUT_ROOT/linux-host"
  local image="$work/backing.img"
  truncate -s "${SIZE_MIB}M" "$image"
  local loop_dev
  loop_dev="$(losetup --find --show --direct-io=on "$image")"
  local sectors mapper crypt_dev
  sectors="$(blockdev --getsz "$loop_dev")"
  mapper="cscc-fio-matrix-$$"
  crypt_dev="/dev/mapper/$mapper"

  cleanup() {
    set +e
    dmsetup remove "$mapper" >/dev/null 2>&1
    losetup -d "$loop_dev" >/dev/null 2>&1
  }
  trap cleanup EXIT

  printf '0 %s crypt aes-xts-plain64 %s 0 %s 0 2 no_read_workqueue no_write_workqueue\n' \
    "$sectors" "$KEY_HEX" "$loop_dev" | dmsetup create "$mapper"

  # Verity on Linux host is optional; mark verity cells N/A unless VERITY_DEV set.
  local verity_dev="${VERITY_DEV:-}"

  fio --name=precondition --filename="$loop_dev" --rw=write --bs=1M \
    --ioengine=libaio --iodepth=32 --direct=1 --size="${SIZE_MIB}M" >/dev/null

  local target operation bs qd run dir device json io_key max_inflight
  for target in "${TARGETS[@]}"; do
    for operation in "${OPERATIONS[@]}"; do
      for bs in "${BLOCK_SIZES[@]}"; do
        qd=1
        if is_verity_write "$target" "$operation"; then
          write_na_row linux-host "$target" "$operation" "$bs" "$qd" "dm-verity_read_only"
          continue
        fi
        if [[ "$target" == "dm-verity" && -z "$verity_dev" ]]; then
          write_na_row linux-host "$target" "$operation" "$bs" "$qd" \
            "set_VERITY_DEV_for_linux_host_verity"
          continue
        fi
        case "$target" in
          raw) device="$loop_dev" ;;
          dm-crypt) device="$crypt_dev" ;;
          dm-verity) device="$verity_dev" ;;
        esac
        dir="$(cell_dir linux-host "$target" "$operation" "$bs" "$qd")"
        write_metadata "$dir" \
          "target=$target" \
          "operation=$operation" \
          "block_size=$bs" \
          "iodepth_set=$qd" \
          "ioengine=libaio" \
          "max_in_flight_observed=unavailable" \
          "fio_version=$(fio --version)" \
          "image_path=$image" \
          "command=sudo bash scripts/run_fio_matrix.sh --platform linux-host"
        printf '%s\n' \
          'platform,target,operation,block_size,iodepth_set,run,status,ioengine,max_in_flight_observed,bw_mib_s,iops,p50_us,p95_us,p99_us,cpu_percent' \
          >"$dir/per-run.csv"
        for run in $(seq 1 "$RUNS"); do
          json="$dir/run-${run}.json"
          fio --name=dm-bench \
            --filename="$device" \
            --rw="$operation" \
            --bs="$bs" \
            --ioengine=libaio \
            --iodepth="$qd" \
            --direct=1 \
            --time_based=1 \
            --runtime="$RUNTIME" \
            --ramp_time="$RAMP" \
            --size="${SIZE_MIB}M" \
            --group_reporting=1 \
            --output-format=json \
            --output="$json"
          if [[ "$operation" == "read" || "$operation" == "randread" ]]; then
            io_key=read
          else
            io_key=write
          fi
          max_inflight="unavailable"
          jq -r --arg platform linux-host \
            --arg target "$target" \
            --arg operation "$operation" \
            --arg bs "$bs" \
            --arg qd "$qd" \
            --arg run "$run" \
            --arg io_key "$io_key" \
            --arg max_inflight "$max_inflight" '
              .jobs[0] as $job |
              $job[$io_key] as $io |
              [
                $platform, $target, $operation, $bs, $qd, $run, "ok", "libaio", $max_inflight,
                ($io.bw_bytes / 1048576),
                $io.iops,
                ($io.clat_ns.percentile["50.000000"] / 1000),
                ($io.clat_ns.percentile["95.000000"] / 1000),
                ($io.clat_ns.percentile["99.000000"] / 1000),
                ($job.usr_cpu + $job.sys_cpu)
              ] | @csv' "$json" >>"$dir/per-run.csv"
        done
      done
    done
  done

  # Extra cell: 4k randread QD=16
  for target in raw dm-crypt; do
    case "$target" in
      raw) device="$loop_dev" ;;
      dm-crypt) device="$crypt_dev" ;;
    esac
    operation=randread
    bs=4k
    qd=16
    dir="$(cell_dir linux-host "$target" "$operation" "$bs" "$qd")"
    write_metadata "$dir" \
      "target=$target" \
      "operation=$operation" \
      "block_size=$bs" \
      "iodepth_set=$qd" \
      "ioengine=libaio" \
      "max_in_flight_observed=unavailable" \
      "note=iodepth_set_is_fio_config_only" \
      "fio_version=$(fio --version)" \
      "command=sudo bash scripts/run_fio_matrix.sh --platform linux-host"
    printf '%s\n' \
      'platform,target,operation,block_size,iodepth_set,run,status,ioengine,max_in_flight_observed,bw_mib_s,iops,p50_us,p95_us,p99_us,cpu_percent' \
      >"$dir/per-run.csv"
    for run in $(seq 1 "$RUNS"); do
      json="$dir/run-${run}.json"
      fio --name=dm-bench \
        --filename="$device" \
        --rw="$operation" \
        --bs="$bs" \
        --ioengine=libaio \
        --iodepth="$qd" \
        --direct=1 \
        --time_based=1 \
        --runtime="$RUNTIME" \
        --ramp_time="$RAMP" \
        --size="${SIZE_MIB}M" \
        --group_reporting=1 \
        --output-format=json \
        --output="$json"
      jq -r --arg platform linux-host \
        --arg target "$target" \
        --arg operation "$operation" \
        --arg bs "$bs" \
        --arg qd "$qd" \
        --arg run "$run" '
          .jobs[0] as $job |
          $job.read as $io |
          [
            $platform, $target, $operation, $bs, $qd, $run, "ok", "libaio", "unavailable",
            ($io.bw_bytes / 1048576),
            $io.iops,
            ($io.clat_ns.percentile["50.000000"] / 1000),
            ($io.clat_ns.percentile["95.000000"] / 1000),
            ($io.clat_ns.percentile["99.000000"] / 1000),
            ($job.usr_cpu + $job.sys_cpu)
          ] | @csv' "$json" >>"$dir/per-run.csv"
    done
  done
  if [[ -z "$verity_dev" ]]; then
    write_na_row linux-host dm-verity randread 4k 16 "set_VERITY_DEV_for_linux_host_verity"
  fi
}

# --- Asterinas KVM path ------------------------------------------------------

run_asterinas_cell() {
  local target="$1" operation="$2" bs="$3" qd="$4"
  local dir script log
  dir="$(cell_dir asterinas-kvm "$target" "$operation" "$bs" "$qd")"
  case "$target" in
    raw|dm-crypt) script="$ASTERINAS_DIR/tools/run_dm_crypt_bench.sh" ;;
    dm-verity) script="$ASTERINAS_DIR/tools/run_dm_verity_bench.sh" ;;
  esac
  if [[ ! -x "$script" && ! -f "$script" ]]; then
    echo "Asterinas bench script missing: $script (kernel tree not ready?)" >&2
    echo "Writing placeholder metadata only." >&2
    write_metadata "$dir" \
      "target=$target" \
      "operation=$operation" \
      "block_size=$bs" \
      "iodepth_set=$qd" \
      "ioengine=sync" \
      "max_in_flight_observed=N/A (sync engine)" \
      "status=pending_asterinas_tree" \
      "command=bash scripts/run_fio_matrix.sh --platform asterinas-kvm"
    return 0
  fi

  write_metadata "$dir" \
    "target=$target" \
    "operation=$operation" \
    "block_size=$bs" \
    "iodepth_set=$qd" \
    "ioengine=sync" \
    "max_in_flight_observed=N/A (sync engine)" \
    "note=iodepth_set_is_fio_config_only_sync_engine_does_not_imply_concurrency" \
    "asterinas_dir=$ASTERINAS_DIR" \
    "command=DM_BENCH_* bash $script"
  log="$dir/serial.log"
  (
    cd "$ASTERINAS_DIR"
    export ENABLE_KVM="${ENABLE_KVM:-1}"
    export SMP="${SMP:-1}"
    export MEM="${MEM:-8G}"
    export DM_BENCH_RW="$operation"
    export DM_BENCH_BS="$bs"
    export DM_BENCH_QD="$qd"
    export DM_BENCH_SIZE="$SIZE_FIO"
    export DM_BENCH_RUNTIME="$RUNTIME"
    export DM_BENCH_RAMP="$RAMP"
    export DM_BENCH_RUNS="$RUNS"
    # For raw cells, crypt bench script still exposes a raw label in matrix mode.
    bash "$script"
  ) | tee "$log"

  python3 "$ROOT_DIR/scripts/summarize_asterinas_dm_matrix.py" \
    "$log" "$dir/matrix-raw.csv" "$dir/matrix-summary.csv" || true

  # Normalize into per-run.csv for summarize_fio_matrix.py
  if [[ -f "$dir/matrix-raw.csv" ]]; then
    python3 - "$dir" "$target" <<'PY'
import csv, sys
from pathlib import Path
cell = Path(sys.argv[1])
target = sys.argv[2]
src = cell / "matrix-raw.csv"
dst = cell / "per-run.csv"
rows = list(csv.DictReader(src.open(encoding="utf-8")))
fields = [
    "platform","target","operation","block_size","iodepth_set","run","status",
    "ioengine","max_in_flight_observed","bw_mib_s","iops","p50_us","p95_us",
    "p99_us","cpu_percent",
]
with dst.open("w", newline="", encoding="utf-8") as out:
    w = csv.DictWriter(out, fieldnames=fields)
    w.writeheader()
    for row in rows:
        w.writerow({
            "platform": "asterinas-kvm",
            "target": target,
            "operation": row["operation"],
            "block_size": row["block_size"],
            "iodepth_set": row["queue_depth"],
            "run": row["run"],
            "status": "ok",
            "ioengine": "sync",
            "max_in_flight_observed": "N/A (sync engine)",
            "bw_mib_s": row["bw_mib_s"],
            "iops": row["iops"],
            "p50_us": row["p50_us"],
            "p95_us": row["p95_us"],
            "p99_us": row["p99_us"],
            "cpu_percent": "unavailable",
        })
print(f"per_run_csv={dst}")
PY
  fi
}

# Split matrix-raw.csv rows into per-target cell directories after a paired boot.
split_asterinas_paired_csv() {
  local paired_dir="$1"
  local operation="$2"
  local bs="$3"
  local qd="$4"
  python3 - "$ROOT_DIR" "$OUT_ROOT" "$paired_dir" "$operation" "$bs" "$qd" <<'PY'
import csv, shutil, sys
from pathlib import Path
root = Path(sys.argv[1])
out_root = Path(sys.argv[2])
paired = Path(sys.argv[3])
operation, bs, qd = sys.argv[4], sys.argv[5], sys.argv[6]
src = paired / "matrix-raw.csv"
if not src.is_file():
    raise SystemExit(0)
rows = list(csv.DictReader(src.open(encoding="utf-8")))
fields = [
    "platform","target","operation","block_size","iodepth_set","run","status",
    "ioengine","max_in_flight_observed","bw_mib_s","iops","p50_us","p95_us",
    "p99_us","cpu_percent",
]
by_label = {}
for row in rows:
    label = row["label"]
    if label in {"raw", "linux-raw"}:
        target = "raw"
    elif "crypt" in label:
        target = "dm-crypt"
    elif "verity" in label:
        target = "dm-verity"
    else:
        target = label
    by_label.setdefault(target, []).append(row)
meta_src = paired / "metadata.txt"
for target, group in by_label.items():
    cell = out_root / "asterinas-kvm" / target / f"{operation}_{bs}_qd{qd}"
    cell.mkdir(parents=True, exist_ok=True)
    if meta_src.is_file():
        text = meta_src.read_text(encoding="utf-8")
        text = text.replace(f"target=dm-crypt", f"target={target}").replace(
            f"target=dm-verity", f"target={target}"
        )
        if "target=" not in text:
            text += f"\ntarget={target}\n"
        (cell / "metadata.txt").write_text(text, encoding="utf-8")
    serial = paired / "serial.log"
    if serial.is_file():
        shutil.copy2(serial, cell / "serial.log")
    with (cell / "per-run.csv").open("w", newline="", encoding="utf-8") as out:
        w = csv.DictWriter(out, fieldnames=fields)
        w.writeheader()
        for row in group:
            w.writerow({
                "platform": "asterinas-kvm",
                "target": target,
                "operation": row["operation"],
                "block_size": row["block_size"],
                "iodepth_set": row["queue_depth"],
                "run": row["run"],
                "status": "ok",
                "ioengine": "sync",
                "max_in_flight_observed": "N/A (sync engine)",
                "bw_mib_s": row["bw_mib_s"],
                "iops": row["iops"],
                "p50_us": row["p50_us"],
                "p95_us": row["p95_us"],
                "p99_us": row["p99_us"],
                "cpu_percent": "unavailable",
            })
    print(f"split_cell={cell}")
PY
}

run_asterinas_matrix() {
  mkdir -p "$OUT_ROOT/asterinas-kvm"
  local operation bs qd paired
  for operation in "${OPERATIONS[@]}"; do
    for bs in "${BLOCK_SIZES[@]}"; do
      qd=1
      # crypt harness: produces raw + dm-crypt rows when available
      paired="$(cell_dir asterinas-kvm _paired-crypt "${operation}" "$bs" "$qd")"
      run_asterinas_cell dm-crypt "$operation" "$bs" "$qd"
      # run_asterinas_cell writes under dm-crypt/; copy to paired then split
      if [[ -d "$(cell_dir asterinas-kvm dm-crypt "$operation" "$bs" "$qd")" ]]; then
        mkdir -p "$paired"
        cp -a "$(cell_dir asterinas-kvm dm-crypt "$operation" "$bs" "$qd")/." "$paired/" 2>/dev/null || true
        split_asterinas_paired_csv "$paired" "$operation" "$bs" "$qd" || true
      fi
      if is_verity_write dm-verity "$operation"; then
        write_na_row asterinas-kvm dm-verity "$operation" "$bs" "$qd" "dm-verity_read_only"
      else
        run_asterinas_cell dm-verity "$operation" "$bs" "$qd"
      fi
    done
  done
  # Extra 4k randread QD=16
  operation=randread
  bs=4k
  qd=16
  paired="$(cell_dir asterinas-kvm _paired-crypt "${operation}" "$bs" "$qd")"
  run_asterinas_cell dm-crypt "$operation" "$bs" "$qd"
  if [[ -d "$(cell_dir asterinas-kvm dm-crypt "$operation" "$bs" "$qd")" ]]; then
    mkdir -p "$paired"
    cp -a "$(cell_dir asterinas-kvm dm-crypt "$operation" "$bs" "$qd")/." "$paired/" 2>/dev/null || true
    split_asterinas_paired_csv "$paired" "$operation" "$bs" "$qd" || true
  fi
  run_asterinas_cell dm-verity "$operation" "$bs" "$qd"
}

mkdir -p "$OUT_ROOT"
rm -f "$OUT_ROOT/na-cells.csv"

case "$PLATFORM" in
  linux-host) run_linux_host_matrix ;;
  asterinas-kvm) run_asterinas_matrix ;;
  *) echo "unknown platform: $PLATFORM" >&2; exit 2 ;;
esac

python3 "$ROOT_DIR/scripts/summarize_fio_matrix.py" "$OUT_ROOT"
echo "fio_matrix_out=$OUT_ROOT"
echo "NOTE: iodepth_set is fio configuration only; see metadata max_in_flight_observed."

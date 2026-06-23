# Performance Protocol

The project now has a reproducible Rust microbenchmark. Final Linux/Asterinas comparison must still be run on the same QEMU configuration and disk image.

## Metrics

- sequential read throughput;
- sequential write throughput for `dm-crypt`;
- random 4 KiB read/write IOPS;
- read latency with and without `dm-verity`;
- boot time with and without protected rootfs.

## Rust Baseline

```bash
BLOCKS=16384 ITERATIONS=7 bash scripts/benchmark.sh target/benchmark/rust.csv
```

The CSV contains plain memory block I/O, AES-256-XTS read/write, and verity read throughput. The benchmark runs mapper construction outside the timed region and includes a read checksum to keep the work observable.

Do not present the memory result as a Linux comparison. It is a stable regression baseline for optimization work.

## Linux/Asterinas Comparison

On a Linux userspace test image:

```bash
fio --name=crypt-test --filename=/mnt/testfile --size=128M --rw=readwrite --bs=4k
fio --name=verity-read --filename=/mnt/testfile --size=128M --rw=read --bs=4k
```

Use identical virtual CPU count, memory, QEMU cache/AIO mode, image, block size, queue depth, runtime and fio version. Record the full command beside every result.

Required matrix:

| OS | Target | Workload | Block size | Queue depth |
| --- | --- | --- | ---: | ---: |
| Linux | plain / dm-crypt / dm-verity | seq read/write, rand read/write | 4KiB | 1 and 32 |
| Asterinas | plain / dm-crypt / dm-verity | same | 4KiB | 1 and 32 |

Report median and spread from at least five measured runs after one warm-up run. Throughput alone is insufficient: retain p50/p95/p99 latency and CPU utilization.

For boot timing, record the elapsed time from QEMU launch to first userspace prompt in two modes:

```text
plain rootfs
dm-verity protected rootfs
```

## Optimization Ideas

- cache hash blocks in the verity path;
- avoid repeated allocation in block transforms;
- batch adjacent block reads when building the verity tree;
- retain AES-NI in production builds and measure software fallback separately;
- reuse per-I/O scratch buffers instead of allocating a `Vec` on every request.

## Report Table Template

| Mode | Read MB/s | Write MB/s | IOPS | Avg Latency | Boot Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain virtio-blk | TBD | TBD | TBD | TBD | TBD |
| dm-crypt | TBD | TBD | TBD | TBD | N/A |
| dm-verity | TBD | N/A | TBD | TBD | TBD |

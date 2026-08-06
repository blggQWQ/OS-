# Completion batching A/B

The semantic before commit is `ed4557fa6`; the after commit is `6b3ca2407`.
Both groups use clean release-LTO builds, KVM, CPU=host, one vCPU, 8 GiB,
256 MiB, 4 KiB reads, 2 s ramp, 10 s runtime and five runs.

- `formal-batch-q1-ab.csv`: QD1 contract-checked comparison.
- `formal-batch-j32-ab.csv`: 32-job contract-checked comparison.
- `after-q1/` and `after-j32/`: serial logs, metadata, per-run fio CSV,
  summaries and stage-attribution CSV files.

The before files are archived in `../formal-256m-worker/after-q1` and
`../formal-256m-worker/after-j32` to avoid duplicating large serial logs.

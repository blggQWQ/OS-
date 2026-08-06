# Per-CPU completion worker A/B

The before commit is the single-worker batching commit `6b3ca2407`; the after
commit is the per-CPU pool commit `9a3483571`. Both sides use clean release-LTO
builds, KVM, CPU=host, four vCPUs, 8 GiB, 256 MiB, 4 KiB reads, 2 s ramp,
10 s runtime and five runs.

- `formal-pool-q1-smp4-ab.csv`: QD1 contract-checked comparison.
- `formal-pool-j32-smp4-ab.csv`: 32-job contract-checked comparison.
- Each before/after directory retains the serial log, metadata, per-run fio
  CSV, summary and stage-attribution CSV.

The pool is beneficial for the 32-job case but is not a universal QD1 win.
That tradeoff is retained rather than hiding the negative normalized result.

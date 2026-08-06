# dm-crypt completion worker A/B evidence

- `before-q1/`: commit `7cb723b87`, five valid QD1 runs.
- `after-q1/`: commit `ed4557fa6`, five valid QD1 runs.
- `after-j32/`: commit `ed4557fa6`, five valid 32-job runs; dm reports `max_in_flight=32`.
- `before-j32-failed/`: commit `7cb723b87`; the first dm-crypt sample stopped making progress after raw run 1. The log and metadata are preserved, but no five-run CSV is claimed.
- `q1-ab.csv`: contract-checked QD1 comparison.

The concurrency contract is `ioengine=sync`, `queue_depth=1`, `jobs=32`. It is intentionally described as 32-job concurrency, not single-job QD32.

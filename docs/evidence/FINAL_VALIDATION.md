# Final KVM validation evidence (2026-07-14)

All guest runs used the official `asterinas/asterinas:0.18.0-20260618` image,
KVM acceleration, and the same 16 MiB protected-root images.

## Functional chain

| Claim | Observable serial evidence | Raw log |
|---|---|---|
| AES-NI dm-crypt root | `AES backend=AES-NI`; protected mapper root switch PASS | `asterinas-crypt-rootfs-aesni-kvm.log` |
| Linux cryptsetup compatibility | cryptsetup 2.7.5 control node, `open` readable mapper, `close` removal all PASS | `asterinas-cryptsetup-aesni-kvm.log` |
| dm-verity positive root | `status=verified`, no data reread, protected root switch PASS | `asterinas-verity-rootfs-kvm.log` |
| dm-verity tamper rejection | `status=integrity-rejected`, mapper absent, zero root-switch PASS lines | `asterinas-verity-rootfs-tampered-kvm.log` |

The tamper case differs from the positive image by exactly one byte: data block
0, offset 0, XOR value `0xff`.

## Five-run cache A/B

The benchmark is 4 KiB fio reads in the KVM guest.  Each configuration records
five raw-device runs and five dm-verity runs.  Compare dm-verity rows only:
the raw-device values are retained as per-boot baselines but are not compared
across guest boots.

| dm-verity cache | Median MiB/s | Median p95 (us) | Median p99 (us) | Final hash hits/misses |
|---:|---:|---:|---:|---:|
| 0 blocks | 0.320 | 15007.744 | 18219.008 | 0 / 2420 |
| 256 blocks | 0.398 | 13697.024 | 18219.008 | 2785 / 5 |

The authenticated cache improves median dm-verity throughput by **24.4%** and
reduces median p95 latency by **8.7%**.  In both cases `data_rereads=0`, so the
result separates hash-cache benefit from duplicate data-block reads.

Raw matrices and summaries:

- `asterinas-verity-fio-cache0-kvm.{log,csv,summary.csv}`
- `asterinas-verity-fio-cache256-kvm.{log,csv,summary.csv}`

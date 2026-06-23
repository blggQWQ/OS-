# Performance Plan

The current crate provides the correctness baseline and reusable mapper logic. Final performance testing should be done after the wrappers are connected to Asterinas block I/O.

## Metrics

- sequential read throughput;
- sequential write throughput for `dm-crypt`;
- random 4 KiB read/write IOPS;
- read latency with and without `dm-verity`;
- boot time with and without protected rootfs.

## Suggested Commands

On a Linux userspace test image:

```bash
fio --name=crypt-test --filename=/mnt/testfile --size=128M --rw=readwrite --bs=4k
fio --name=verity-read --filename=/mnt/testfile --size=128M --rw=read --bs=4k
```

Or use:

```bash
scripts/benchmark.sh /mnt/dm-test 128M
```

For boot timing, record the elapsed time from QEMU launch to first userspace prompt in two modes:

```text
plain rootfs
dm-verity protected rootfs
```

## Optimization Ideas

- cache hash blocks in the verity path;
- avoid repeated allocation in block transforms;
- batch adjacent block reads when building the verity tree;
- replace the demo cipher with hardware-accelerated AES-XTS when available.

## Report Table Template

| Mode | Read MB/s | Write MB/s | IOPS | Avg Latency | Boot Time |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain virtio-blk | TBD | TBD | TBD | TBD | TBD |
| dm-crypt | TBD | TBD | TBD | TBD | N/A |
| dm-verity | TBD | N/A | TBD | TBD | TBD |

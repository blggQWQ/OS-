# Test Plan / 测试说明

## Local Rust Tests

Run:

```powershell
cargo test
```

Covered cases:

| Case | Expected Result |
| --- | --- |
| `dm-crypt` write then read | Returned data equals plaintext |
| Direct lower-device read after encrypted write | Data differs from plaintext |
| Wrong key read | Data does not match plaintext |
| `dm-verity` normal read | Verification passes |
| Tampered data block | Read returns `IntegrityViolation` |
| Wrong root hash | Read returns `IntegrityViolation` |
| `dm-verity` write | Write returns `ReadOnlyDevice` |

## QEMU / Asterinas Tests To Add

1. Boot Asterinas in QEMU.
2. Attach a raw `test.img` with `if=virtio`.
3. Confirm the OS can discover the disk.
4. Run passthrough read/write.
5. Run encrypted write through `DmCryptDevice` and inspect raw disk bytes.
6. Build a verity root hash for a read-only image.
7. Tamper with the image and confirm protected reads fail.

## Evidence For Submission

- QEMU boot log.
- Screenshot showing virtio-blk detection.
- Screenshot/log showing encrypted lower bytes.
- Screenshot/log showing verity failure after tampering.
- Test command output.

## CLI Demo Tests

Run:

```powershell
cargo run -p dmctl -- demo-crypt
cargo run -p dmctl -- demo-verity
```

Expected `demo-crypt` evidence:

- `plain` contains readable plaintext;
- `raw-hex` differs from plaintext;
- `read` equals plaintext;
- `ciphertext_differs: true`.

Expected `demo-verity` evidence:

- root hash is printed;
- tampered block read reports `tamper_detected: true`.

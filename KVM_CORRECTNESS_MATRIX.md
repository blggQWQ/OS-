# KVM correctness matrix

Baseline: Asterinas `9d79542cadddc6164ad3f31fd8cba776969d85a9`, image digest `sha256:7510071cb103309cc9396b970325c354713da749dfee34a807108248cc97331e`, Cargo volume `asterinas-cargo`, rustup volume `asterinas-rustup`, x86_64, QEMU `-accel kvm -cpu host`.

| ID | 模块 | 测试 | 结果 | 测试环境 | Evidence | 备注 |
|---|---|---|---|---|---|---|
| R01 | regression | cryptsetup smoke | PASS | fixed digest + Cargo offline + persistent rustup + KVM | `docs/evidence/stabilization/correctness-kvm/regression/R01-cryptsetup-persistent-cache.log` | real cryptsetup open/read/close PASS；无 Cargo/rustup 下载 |
| R02 | regression | dm-crypt rootfs smoke | PASS | same baseline | `docs/evidence/stabilization/correctness-kvm/regression/R02-crypt-rootfs-persistent-cache.log` | protected root switch 与 initramfs retention PASS |
| R03 | regression | dm-verity rootfs smoke | PASS | same baseline | `docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log` | verified、protected root switch 与 initramfs retention PASS |
| C01 | dm-crypt | correct key mapper read/write | PASS | Asterinas KVM + real cryptsetup mapper | `docs/evidence/stabilization/correctness-kvm/crypt/C01-C05-kvm.log` | actual mapper write/read hash equal |
| C02 | dm-crypt | wrong key | PASS | same | same | correct hash `5e9f…a358` != wrong-key hash `2aef…1833` |
| C03 | dm-crypt | 4K write/read | PASS | same | same | write/read SHA-256 equal |
| C04 | dm-crypt | 32K write/read | PASS | same | same | write/read SHA-256 equal |
| C05 | dm-crypt | 256K write/read | PASS | same | same | write/read SHA-256 equal |
| C06-C10 | dm-crypt | multi-block/boundary/segments/raw/lifecycle | UNTESTED | no existing mode | None | 未执行 |
| C11-C15 | dm-crypt | invalid input / range | UNTESTED | no existing mode | None | 未执行 |
| V01 | dm-verity | normal image + correct root | PASS | fixed baseline + KVM | `docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log` | `status=verified` 且 protected root switch PASS |
| V02 | dm-verity | wrong root hash | PASS | unchanged image, altered cmdline root only | `docs/evidence/stabilization/correctness-kvm/corruption/V02-wrong-root.log` | `status=integrity-rejected`；mapper missing；无 protected-root PASS、panic 或 fallback |
| V03 | dm-verity | first data block corruption | PASS | targeted mapper probe | `docs/evidence/stabilization/correctness-kvm/corruption/V03-targeted-probe.log` | rejected before mapper publication |
| V04 | dm-verity | middle data block corruption | PASS | targeted block 32768 read | `docs/evidence/stabilization/correctness-kvm/corruption/V04-targeted-probe.log` | mapper read returned `Input/output error` |
| V05 | dm-verity | last data block corruption | PASS | targeted block 65535 read | `docs/evidence/stabilization/correctness-kvm/corruption/V05-targeted-probe.log` | mapper read returned `Input/output error` |
| V06 | dm-verity | leaf hash block corruption | PASS | targeted mapper probe | `docs/evidence/stabilization/correctness-kvm/corruption/V06-targeted-probe.log` | rejected before mapper publication |
| V07 | dm-verity | intermediate hash corruption | PASS | targeted mapper probe | `docs/evidence/stabilization/correctness-kvm/corruption/V07-targeted-probe.log` | rejected before mapper publication |
| V08 | dm-verity | last hash-tree block | UNTESTED | P1 | None | P1 pending |
| V09-V12 | dm-verity | boundary / 32K / 256K / large BIO | UNTESTED | no existing mode | None | 未执行 |
| H01-H08 | hash cache | capacities / eviction / corruption invariant | UNTESTED | P1 | None | P1 pending |
| P01 | persistence | dm-crypt reboot/reopen | UNTESTED | P1 | None | P1 pending |

结果只使用 `PASS`、`FAIL`、`UNTESTED`、`BLOCKED`、`CODE-VERIFIED`。当前联网 correctness 环境可用；Nix 完全离线复现问题单独记录，不记为产品 FAIL。

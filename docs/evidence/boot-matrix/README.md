# Boot 时间矩阵证据

定义：QEMU 进程启动（host monotonic）→ 串口首次出现

`PASS system root switched to protected mapper`

场景：`plain`、`dm-crypt`、`dm-verity`，各 ≥5 轮，交替顺序由驱动控制。

## 状态

| 场景 | 状态 |
|---|---|
| plain | 待跑 |
| dm-crypt | 待跑 |
| dm-verity | 待跑 |

## 生成

```bash
# 先按文档构建 plain / crypt / verity boot ISO 与镜像
bash scripts/build_asterinas_boot_images.sh   # 容器内
bash scripts/run_boot_matrix.sh
```

输出：`docs/evidence/boot-matrix/boot-time.csv`、
`boot-time-summary.csv`、`logs/{scenario}-run-{n}.log`。

#!/usr/bin/env python3
"""Plot the published Linux guest vs Asterinas 4 KiB sync QD1 comparison table."""

from __future__ import annotations

from pathlib import Path

import matplotlib as mpl
import matplotlib.pyplot as plt
import pandas as pd
from matplotlib import font_manager

ROOT = Path(__file__).resolve().parents[1]
CSV_PATH = ROOT / "scripts" / "data" / "linux_asterinas_4k_sync.csv"
OUT_DIR = ROOT / "docs" / "design_doc" / "figures"

LINUX_COLOR = "#0072B2"
ASTERINAS_COLOR = "#E69F00"
TARGETS = ("raw", "dm-crypt", "dm-verity")
TARGET_LABELS = {
    "raw": "raw",
    "dm-crypt": "dm-crypt",
    "dm-verity": "dm-verity",
}


def pick_cjk_font() -> str | None:
    preferred = (
        "Noto Sans CJK SC",
        "Noto Sans CJK JP",
        "WenQuanYi Micro Hei",
        "Noto Serif CJK SC",
        "AR PL UMing CN",
    )
    available = {f.name for f in font_manager.fontManager.ttflist}
    for name in preferred:
        if name in available:
            return name
    return None


def setup_style() -> None:
    font = pick_cjk_font()
    mpl.rcParams.update(
        {
            "figure.facecolor": "white",
            "axes.facecolor": "white",
            "axes.edgecolor": "#333333",
            "axes.grid": True,
            "grid.color": "#dddddd",
            "grid.linestyle": "--",
            "axes.axisbelow": True,
            "font.size": 11,
            "axes.titlesize": 13,
            "axes.labelsize": 11,
            "pdf.fonttype": 42,
            "ps.fonttype": 42,
        }
    )
    if font:
        mpl.rcParams["font.family"] = font
        mpl.rcParams["axes.unicode_minus"] = False


def load_table() -> pd.DataFrame:
    df = pd.read_csv(CSV_PATH)
    return df.set_index(["platform", "target"])


def save(fig: plt.Figure, stem: str) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    png = OUT_DIR / f"{stem}.png"
    pdf = OUT_DIR / f"{stem}.pdf"
    fig.savefig(png, dpi=150, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    print(f"wrote {png}")
    print(f"wrote {pdf}")


def plot_throughput(df: pd.DataFrame) -> None:
    x = list(range(len(TARGETS)))
    width = 0.36
    linux = [float(df.loc[("Linux guest", t), "median_mib_s"]) for t in TARGETS]
    aster = [float(df.loc[("Asterinas", t), "median_mib_s"]) for t in TARGETS]

    fig, ax = plt.subplots(figsize=(7.2, 4.4))
    bars_l = ax.bar(
        [i - width / 2 for i in x],
        linux,
        width,
        label="Linux guest",
        color=LINUX_COLOR,
    )
    bars_a = ax.bar(
        [i + width / 2 for i in x],
        aster,
        width,
        label="Asterinas",
        color=ASTERINAS_COLOR,
    )
    ax.set_xticks(x)
    ax.set_xticklabels([TARGET_LABELS[t] for t in TARGETS])
    ax.set_ylabel("中位吞吐（MiB/s）")
    ax.set_title("4 KiB sync QD1，256 MiB virtio-blk：Linux guest 与 Asterinas")
    ax.set_ylim(0, max(linux + aster) * 1.28)
    ax.legend(loc="upper right", frameon=True, fancybox=False, edgecolor="#cccccc")

    for bars in (bars_l, bars_a):
        for bar in bars:
            height = bar.get_height()
            ax.annotate(
                f"{height:.3f}",
                xy=(bar.get_x() + bar.get_width() / 2, height),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=9,
            )
    fig.tight_layout()
    save(fig, "linux_asterinas_4k_sync_throughput")
    plt.close(fig)


def plot_retention(df: pd.DataFrame) -> None:
    labels = ["dm-crypt\n（相对表中 raw）", "dm-verity\n（相对配对 raw）"]
    linux = [
        float(df.loc[("Linux guest", "dm-crypt"), "retention_percent"]),
        float(df.loc[("Linux guest", "dm-verity"), "retention_percent"]),
    ]
    aster = [
        float(df.loc[("Asterinas", "dm-crypt"), "retention_percent"]),
        float(df.loc[("Asterinas", "dm-verity"), "retention_percent"]),
    ]
    x = [0, 1]
    width = 0.36

    fig, ax = plt.subplots(figsize=(7.2, 4.6))
    bars_l = ax.bar(
        [i - width / 2 for i in x],
        linux,
        width,
        label="Linux guest",
        color=LINUX_COLOR,
    )
    bars_a = ax.bar(
        [i + width / 2 for i in x],
        aster,
        width,
        label="Asterinas",
        color=ASTERINAS_COLOR,
    )
    ax.set_xticks(x)
    ax.set_xticklabels(labels)
    ax.set_ylabel("相对自身基线的保留率（%）")
    ax.set_title(
        "Mapper 保留率：crypt 相对表中 raw；verity 相对配对 raw\n"
        "（Linux 配对 raw 4.641，Asterinas 配对 raw 4.368）"
    )
    ax.legend(loc="upper right", frameon=True, fancybox=False, edgecolor="#cccccc")
    ax.set_ylim(0, 115)
    ax.axhline(100, color="#888888", linewidth=0.8)

    for bars in (bars_l, bars_a):
        for bar in bars:
            height = bar.get_height()
            ax.annotate(
                f"{height:.3f}".rstrip("0").rstrip(".") + "%",
                xy=(bar.get_x() + bar.get_width() / 2, height),
                xytext=(0, 3),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=9,
            )
    fig.tight_layout()
    save(fig, "linux_asterinas_4k_sync_retention")
    plt.close(fig)


def main() -> None:
    setup_style()
    df = load_table()
    plot_throughput(df)
    plot_retention(df)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "matplotlib>=3.10.8",
# ]
# ///
"""
Plot benchmark history (time, throughput, CPU, RAM) with fzf baseline.

Usage:
  ./scripts/plot-bench.py [--history PATH] [--baseline PATH] [--out PATH]
                          [--since YYYY-MM-DD]

Defaults:
  history: ./scripts/data/bench-history.json
  baseline: ./scripts/data/fzf-baseline.json
  out: ./scripts/data/bench-plots.png
  since: 2026-01-01

The script expects `history` to be a JSON array where each element is an
object produced by scripts/bench-history.py (with keys: version, date, bench).
Each bench entry should follow the bench.sh JSON layout (time_s, peak_cpu,
peak_memory_kb, items_per_second) with avg/min/max values.

The plot shows min/avg/max bands and a horizontal baseline from the fzf
benchmark file.  Row 1: time (s) and throughput (items/s).  Row 2: CPU % and
memory (MB).
"""

from __future__ import annotations

import argparse
import json
import os
from datetime import datetime, timezone
from typing import List, Dict, Any, Optional, Tuple

import matplotlib.pyplot as plt
import matplotlib.dates as mdates


def load_json(path: str) -> Any:
    with open(path, "r") as f:
        return json.load(f)


def safe_get(d: Dict[str, Any], *keys, default=None):
    cur = d
    for k in keys:
        if cur is None:
            return default
        cur = cur.get(k)
    return cur if cur is not None else default


def _to_float_or_none(v):
    if v is None:
        return None
    try:
        return float(v)
    except Exception:
        return None


def parse_history(
    history: List[Dict[str, Any]],
    since: Optional[datetime] = None,
) -> Tuple[List[str], List[datetime], Dict[str, Dict[str, List[Optional[float]]]]]:
    """Sort history by date, optionally filter to entries >= since, return plotting data."""

    def parse_date(r):
        try:
            dt = datetime.fromisoformat(r.get("date").replace("Z", "+00:00"))
            return dt
        except Exception:
            return datetime.min.replace(tzinfo=timezone.utc)

    history_sorted = sorted(history, key=parse_date)

    if since is not None:
        if since.tzinfo is None:
            since = since.replace(tzinfo=timezone.utc)
        history_sorted = [r for r in history_sorted if parse_date(r) >= since]

    versions = [r.get("version") or f"#{i}" for i, r in enumerate(history_sorted)]
    dates = [parse_date(r) for r in history_sorted]

    metrics: Dict[str, Dict[str, List[Optional[float]]]] = {
        "time": {"avg": [], "min": [], "max": []},
        "throughput": {"avg": [], "min": [], "max": []},
        "cpu": {"avg": [], "min": [], "max": []},
        "mem": {"avg": [], "min": [], "max": []},
    }

    for r in history_sorted:
        b = r.get("bench") or {}

        metrics["time"]["avg"].append(_to_float_or_none(safe_get(b, "time_s", "avg")))
        metrics["time"]["min"].append(_to_float_or_none(safe_get(b, "time_s", "min")))
        metrics["time"]["max"].append(_to_float_or_none(safe_get(b, "time_s", "max")))

        metrics["throughput"]["avg"].append(
            _to_float_or_none(safe_get(b, "items_per_second", "avg"))
        )
        metrics["throughput"]["min"].append(
            _to_float_or_none(safe_get(b, "items_per_second", "min"))
        )
        metrics["throughput"]["max"].append(
            _to_float_or_none(safe_get(b, "items_per_second", "max"))
        )

        metrics["cpu"]["avg"].append(
            _to_float_or_none(safe_get(b, "peak_cpu", "avg") or safe_get(b, "peak_cpu"))
        )
        metrics["cpu"]["min"].append(_to_float_or_none(safe_get(b, "peak_cpu", "min")))
        metrics["cpu"]["max"].append(_to_float_or_none(safe_get(b, "peak_cpu", "max")))

        metrics["mem"]["avg"].append(
            _to_float_or_none(safe_get(b, "peak_memory_kb", "avg"))
        )
        metrics["mem"]["min"].append(
            _to_float_or_none(safe_get(b, "peak_memory_kb", "min"))
        )
        metrics["mem"]["max"].append(
            _to_float_or_none(safe_get(b, "peak_memory_kb", "max"))
        )

    return versions, dates, metrics


def plot_band(ax, x_nums, y_min, y_avg, y_max, label: str, color: str):
    """Plot an avg line with a min/max shaded band, skipping None values."""
    import math

    y_min_f = [math.nan if v is None else float(v) for v in y_min]
    y_avg_f = [math.nan if v is None else float(v) for v in y_avg]
    y_max_f = [math.nan if v is None else float(v) for v in y_max]

    ax.plot(
        x_nums, y_avg_f, label=label + " (avg)", color=color, marker="o", markersize=4
    )
    ax.fill_between(
        x_nums, y_min_f, y_max_f, color=color, alpha=0.2, label=label + " (min-max)"
    )


def _is_minor_release(version: str) -> bool:
    """Return True iff version is a minor release (patch == 0, or -pre1 suffix).

    Examples:
        v1.2.0      -> True
        v1.2.1      -> False
        v1.0.0-pre3 -> False
        #3          -> False
    """
    import re

    if version == "HEAD":
        return True

    m = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)(.*)", version)
    if not m:
        return False
    return int(m.group(3)) == 0 and (m.group(4) == "" or m.group(4) == "-pre1")


def apply_date_xaxis(ax, x_nums, versions):
    """Configure the x-axis: tick at every data point, label only minor releases."""
    ax.set_xticks(x_nums)
    labels = [v if _is_minor_release(v) else "" for v in versions]
    ax.set_xticklabels(labels, rotation=45, ha="right", fontsize=8)
    # Keep minor tick marks visible for unlabelled points without a label
    ax.tick_params(axis="x", which="major", length=4)
    # Draw a longer tick for labelled (minor release) positions
    for tick, label in zip(ax.xaxis.get_major_ticks(), labels):
        if label:
            tick.tick1line.set_markersize(8)


def prepare_baseline(b: Dict[str, Any]) -> Dict[str, Tuple]:
    return {
        "time": (
            _to_float_or_none(safe_get(b, "time_s", "avg")),
            _to_float_or_none(safe_get(b, "time_s", "min")),
            _to_float_or_none(safe_get(b, "time_s", "max")),
        ),
        "throughput": (
            _to_float_or_none(safe_get(b, "items_per_second", "avg")),
            _to_float_or_none(safe_get(b, "items_per_second", "min")),
            _to_float_or_none(safe_get(b, "items_per_second", "max")),
        ),
        "cpu": (
            _to_float_or_none(safe_get(b, "peak_cpu", "avg")),
            _to_float_or_none(safe_get(b, "peak_cpu", "min")),
            _to_float_or_none(safe_get(b, "peak_cpu", "max")),
        ),
        "mem": (
            _to_float_or_none(safe_get(b, "peak_memory_kb", "avg")),
            _to_float_or_none(safe_get(b, "peak_memory_kb", "min")),
            _to_float_or_none(safe_get(b, "peak_memory_kb", "max")),
        ),
    }


def add_baseline_hline(ax, baseline_tuple, scale=1.0):
    """Draw a horizontal dashed line for the fzf baseline if available."""
    if baseline_tuple is None:
        return
    b_avg, b_min, b_max = baseline_tuple
    if b_avg is None:
        return
    ax.axhline(
        b_avg * scale, color="k", linestyle="--", linewidth=1, label="fzf baseline"
    )
    if b_min is not None and b_max is not None:
        # light band for baseline min/max — we use axhspan via dummy x range
        ax.axhspan(b_min * scale, b_max * scale, color="k", alpha=0.08)


def main(argv=None):
    p = argparse.ArgumentParser()
    p.add_argument("--history", default="./scripts/data/bench-history.json")
    p.add_argument("--baseline", default="./scripts/data/fzf-baseline.json")
    p.add_argument("--out", default="./scripts/data/bench-plots.png")
    p.add_argument(
        "--since",
        default="2026-01-01",
        help="Only show data on or after this date (YYYY-MM-DD). Pass '' to disable.",
    )
    args = p.parse_args(argv)

    since: Optional[datetime] = None
    if args.since:
        since = datetime.strptime(args.since, "%Y-%m-%d").replace(tzinfo=timezone.utc)

    history_path = args.history
    baseline_path = args.baseline
    out_path = args.out

    if not os.path.isfile(history_path):
        raise SystemExit(f"history file not found: {history_path}")
    history = load_json(history_path)

    baseline = None
    if os.path.isfile(baseline_path):
        baseline = prepare_baseline(load_json(baseline_path))
    else:
        print(
            f"warning: baseline file not found: {baseline_path} — proceeding without baseline"
        )

    versions, dates, metrics = parse_history(history, since=since)
    if not versions:
        raise SystemExit("No data points after the --since filter.")

    x_nums = mdates.date2num(dates)

    # MB conversion for memory
    mem_avg_mb = [None if v is None else v / 1024.0 for v in metrics["mem"]["avg"]]
    mem_min_mb = [None if v is None else v / 1024.0 for v in metrics["mem"]["min"]]
    mem_max_mb = [None if v is None else v / 1024.0 for v in metrics["mem"]["max"]]

    # Throughput in millions of items/s for readability
    tp_scale = 1e6
    tp_avg = [None if v is None else v / tp_scale for v in metrics["throughput"]["avg"]]
    tp_min = [None if v is None else v / tp_scale for v in metrics["throughput"]["min"]]
    tp_max = [None if v is None else v / tp_scale for v in metrics["throughput"]["max"]]

    n = len(versions)
    fig_w = max(14, n * 0.55)
    fig, axes = plt.subplots(2, 2, figsize=(fig_w, 9), sharex=True)
    ax_time, ax_tp, ax_cpu, ax_mem = axes[0, 0], axes[0, 1], axes[1, 0], axes[1, 1]

    # --- Row 1, col 0: Time ---
    plot_band(
        ax_time,
        x_nums,
        metrics["time"]["min"],
        metrics["time"]["avg"],
        metrics["time"]["max"],
        "Time",
        "C0",
    )
    add_baseline_hline(ax_time, baseline.get("time") if baseline else None)
    ax_time.set_ylabel("Time (s)")
    ax_time.legend(fontsize=8)
    ax_time.set_ylim(bottom=0)
    ax_time.set_title("Execution time (lower is better)")

    # --- Row 1, col 1: Throughput ---
    plot_band(ax_tp, x_nums, tp_min, tp_avg, tp_max, "Throughput", "C1")
    if (
        baseline
        and baseline.get("throughput")
        and baseline["throughput"][0] is not None
    ):
        add_baseline_hline(
            ax_tp,
            tuple(None if v is None else v / tp_scale for v in baseline["throughput"]),
        )
    ax_tp.set_ylabel("Throughput (M items/s)")
    ax_tp.legend(fontsize=8)
    ax_tp.set_ylim(bottom=0)
    ax_tp.set_title("Throughput (higher is better)")

    # --- Row 2, col 0: CPU ---
    plot_band(
        ax_cpu,
        x_nums,
        metrics["cpu"]["min"],
        metrics["cpu"]["avg"],
        metrics["cpu"]["max"],
        "CPU %",
        "tab:orange",
    )
    add_baseline_hline(ax_cpu, baseline.get("cpu") if baseline else None)
    ax_cpu.set_ylabel("CPU %")
    ax_cpu.legend(fontsize=8)
    ax_cpu.set_ylim(bottom=0)
    ax_cpu.set_title("Peak CPU usage (lower is better)")

    # --- Row 2, col 1: Memory ---
    plot_band(
        ax_mem,
        x_nums,
        mem_min_mb,
        mem_avg_mb,
        mem_max_mb,
        "Memory",
        "tab:green",
    )
    if baseline and baseline.get("mem") and baseline["mem"][0] is not None:
        add_baseline_hline(
            ax_mem, tuple(None if v is None else v / 1024.0 for v in baseline["mem"])
        )
    ax_mem.set_ylabel("Memory (MB)")
    ax_mem.legend(fontsize=8)
    ax_mem.set_ylim(bottom=0)
    ax_mem.set_title("Peak memory usage (lower is better)")

    # --- X-axis formatting: date-scaled, version labels on bottom row ---
    for ax in (ax_cpu, ax_mem):
        apply_date_xaxis(ax, x_nums, versions)

    fig.suptitle("skim benchmark history", fontsize=13, fontweight="bold")
    plt.tight_layout()

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    plt.savefig(out_path, dpi=150)
    print(f"wrote plots to {out_path}")


if __name__ == "__main__":
    main()

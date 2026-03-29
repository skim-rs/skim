#!/usr/bin/env python3
"""
Benchmark script to measure ingestion + matching rate in skim interactive mode.
This measures how fast skim can ingest items and display matched results.

Usage: bench.py [BINARY_PATH ...] [-n|--num-items NUM] [-q|--query QUERY]
                [-r|--runs RUNS] [-w|--warmup N] [-f|--file FILE]
                [-g|--generate-file FILE] [-j|--json] [-p|--perf [FILE]]
                [-- EXTRA_ARGS...]

Arguments:
  BINARY_PATH ...          One or more paths to binaries (default: ./target/release/sk)
                           When multiple are given they run in round-robin and the
                           first is used as the baseline for +/- comparisons.
  -n, --num-items NUM      Number of items to generate (default: 1000000)
  -q, --query QUERY        Query string to search (default: "test")
  -r, --runs RUNS          Number of benchmark runs per binary (default: 1)
  -w, --warmup N           Number of warmup runs per binary (default: 1)
  -f, --file FILE          Use existing file as input instead of generating
  -g, --generate-file FILE Generate test data to file and exit
  -j, --json               Output results as JSON
  --                       Pass remaining arguments to the binary

Examples:
  ./bench.py                                         # Use defaults
  ./bench.py ./target/release/sk -n 500000 -q foo
  ./bench.py ./old/sk ./new/sk -r 5                 # Compare two binaries
  ./bench.py -r 5                                    # Run 5 times and show average
  ./bench.py -f input.txt -q search                 # Use existing file
  ./bench.py -g testdata.txt -n 2000000             # Generate file and exit
  ./bench.py -p                                      # Record perf data (auto-named file)
  ./bench.py -p perf.data                            # Record perf data to perf.data
"""

import json
import math
import os
import random
import subprocess
import sys
import tempfile
import time

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
DEFAULT_BINARY = "./target/release/sk"
DEFAULT_NUM_ITEMS = 1_000_000
DEFAULT_QUERY = "test"
DEFAULT_RUNS = 1
DEFAULT_WARMUP = 1

# Stability / timeout tuning (mirrors bench.sh values)
REQUIRED_STABLE_S = 5.0  # seconds the matched count must be unchanged
MAX_WAIT_S = 60.0  # hard timeout per run
CHECK_INTERVAL_S = 0.05  # polling interval


# ---------------------------------------------------------------------------
# Test-data generation
# ---------------------------------------------------------------------------

WORDS = [
    "home",
    "usr",
    "etc",
    "var",
    "opt",
    "tmp",
    "dev",
    "proc",
    "sys",
    "lib",
    "bin",
    "sbin",
    "boot",
    "mnt",
    "media",
    "src",
    "test",
    "config",
    "data",
    "logs",
    "cache",
    "backup",
    "docs",
    "images",
    "videos",
    "audio",
    "downloads",
    "uploads",
    "temp",
    "shared",
]


def generate_test_data(output_file: str, num_items: int) -> None:
    rng = random.Random()
    with open(output_file, "w") as fh:
        for i in range(1, num_items + 1):
            depth = rng.randint(2, 10)
            parts = [rng.choice(WORDS) for _ in range(depth)]
            fh.write("/".join(parts) + f"_{i}\n")


# ---------------------------------------------------------------------------
# Argument parsing  (no argparse – stdlib only, but argparse IS stdlib…
# however, to keep the spirit of "no dependencies" we use manual parsing
# since argparse is a stdlib module, not a third-party dep. We use it.)
# ---------------------------------------------------------------------------


def parse_args(argv):
    """Return (binaries, opts, extra_args)."""
    import argparse

    # Split off everything after "--"
    extra_args = []
    if "--" in argv:
        sep_idx = argv.index("--")
        extra_args = argv[sep_idx + 1 :]
        argv = argv[:sep_idx]

    parser = argparse.ArgumentParser(
        description="Skim benchmark script",
        add_help=True,
    )
    parser.add_argument(
        "binaries",
        nargs="*",
        metavar="BINARY_PATH",
        help="Path(s) to binary (default: ./target/release/sk)",
    )
    parser.add_argument("-n", "--num-items", type=int, default=DEFAULT_NUM_ITEMS)
    parser.add_argument("-q", "--query", default=DEFAULT_QUERY)
    parser.add_argument("-r", "--runs", type=int, default=DEFAULT_RUNS)
    parser.add_argument("-w", "--warmup", type=int, default=DEFAULT_WARMUP)
    parser.add_argument("-f", "--file", default="")
    parser.add_argument("-g", "--generate-file", default="")
    parser.add_argument("-j", "--json", action="store_true")
    parser.add_argument(
        "-p",
        "--perf",
        nargs="?",
        const="",  # flag present but no value → auto-name
        default=None,  # flag absent
        metavar="FILE",
        help="Record perf data for the final benchmark run. "
        "Optionally specify output file path (default: auto-named perf-<binary>-<ts>.data).",
    )

    opts = parser.parse_args(argv)

    if not opts.binaries:
        opts.binaries = [DEFAULT_BINARY]

    if opts.file and opts.generate_file:
        parser.error("Cannot use both --file and --generate-file")

    return opts.binaries, opts, extra_args


# ---------------------------------------------------------------------------
# Resource monitor (background thread)
# ---------------------------------------------------------------------------

import threading


class ResourceMonitor(threading.Thread):
    """Sample CPU and RSS of *pid* every 50 ms until the process exits."""

    def __init__(self, pid: int):
        super().__init__(daemon=True)
        self.pid = pid
        self.peak_mem_kb: int = 0  # RSS in kB
        self.peak_cpu: float = 0.0  # %CPU

    def run(self):
        while True:
            try:
                result = subprocess.run(
                    ["ps", "-p", str(self.pid), "-o", "rss=,%cpu="],
                    capture_output=True,
                    text=True,
                )
                line = result.stdout.strip()
                if not line:
                    break
                parts = line.split()
                if len(parts) >= 2:
                    try:
                        mem = int(parts[0])
                        cpu = float(parts[1])
                        if mem > self.peak_mem_kb:
                            self.peak_mem_kb = mem
                        if cpu > self.peak_cpu:
                            self.peak_cpu = cpu
                    except ValueError:
                        pass
            except Exception:
                break
            time.sleep(0.05)


# ---------------------------------------------------------------------------
# Single run
# ---------------------------------------------------------------------------


def _find_sk_pid(pane_pid: int, binary_path: str) -> int:
    """Try for up to 5 s to find the sk child PID under *pane_pid*."""
    for _ in range(50):
        time.sleep(0.1)
        try:
            result = subprocess.run(
                ["pgrep", "-P", str(pane_pid), "-f", binary_path],
                capture_output=True,
                text=True,
            )
            pids = result.stdout.strip().splitlines()
            if pids:
                return int(pids[0])
        except Exception:
            pass
    return 0


def run_once(
    binary_path: str,
    query: str,
    tmp_file: str,
    num_items: int,
    extra_args: list,
    run_index: int,
    session_suffix: str,
    perf_output: str | None = None,
) -> dict:
    """
    Execute one benchmark run against *binary_path*.
    Returns a dict with keys: elapsed_s, rate, matched, peak_mem_kb, peak_cpu,
    completed, perf_file (path or None).
    If *perf_output* is a non-empty string, ``perf record`` is attached to the
    sk process and data written to that path.
    """
    session_name = f"skim_bench_{os.getpid()}_{session_suffix}_{run_index}"
    status_fd, status_file = tempfile.mkstemp(prefix="skim_bench_status_")
    os.close(status_fd)

    env = os.environ.copy()
    env["SHELL"] = "/bin/sh"
    env.pop("HISTFILE", None)
    env.pop("FZF_DEFAULT_OPTS", None)
    env.pop("SKIM_DEFAULT_OPTIONS", None)

    try:
        # Create tmux session
        subprocess.run(
            ["tmux", "new-session", "-s", session_name, "-d"],
            check=True,
            env=env,
            capture_output=True,
        )

        # Clear env vars inside the session
        for cmd in [
            "unset HISTFILE",
            "unset FZF_DEFAULT_OPTS",
            "unset SKIM_DEFAULT_OPTIONS",
        ]:
            subprocess.run(
                ["tmux", "send-keys", "-t", session_name, cmd, "Enter"],
                check=True,
                capture_output=True,
            )
        time.sleep(0.1)

        # Build the command string
        extra_str = " ".join(extra_args)
        perf_prefix = f"perf record -o {perf_output} -- " if perf_output else ""
        cmd_str = f"cat {tmp_file} | {perf_prefix}{binary_path} --query '{query}' {extra_str}"
        subprocess.run(
            ["tmux", "send-keys", "-t", session_name, cmd_str, "Enter"],
            check=True,
            capture_output=True,
        )

        start_ns = time.perf_counter_ns()

        # Locate sk PID for resource monitoring
        pane_pid = 0
        try:
            r = subprocess.run(
                ["tmux", "list-panes", "-t", session_name, "-F", "#{pane_pid}"],
                capture_output=True,
                text=True,
            )
            pane_pid = int(r.stdout.strip().splitlines()[0])
        except Exception:
            pass

        sk_pid = 0
        monitor = None
        if pane_pid:
            sk_pid = _find_sk_pid(pane_pid, binary_path)
        if sk_pid:
            monitor = ResourceMonitor(sk_pid)
            monitor.start()

        # Poll for matcher completion
        completed = False
        matched_count = 0
        prev_matched_count = -1
        stable_start: float = 0.0
        end_ns = 0
        loop_start = time.monotonic()

        while True:
            time.sleep(CHECK_INTERVAL_S)

            now = time.monotonic()
            if now - loop_start >= MAX_WAIT_S:
                break

            # Early exit if sk process is gone
            if sk_pid:
                try:
                    os.kill(sk_pid, 0)
                except ProcessLookupError:
                    break

            # Capture tmux pane
            try:
                subprocess.run(
                    [
                        "tmux",
                        "capture-pane",
                        "-b",
                        f"status-{session_name}",
                        "-t",
                        session_name,
                    ],
                    capture_output=True,
                )
                subprocess.run(
                    [
                        "tmux",
                        "save-buffer",
                        "-b",
                        f"status-{session_name}",
                        status_file,
                    ],
                    capture_output=True,
                )
            except Exception:
                continue

            # Parse "matched/total" from status line
            try:
                with open(status_file) as fh:
                    content = fh.read()
            except OSError:
                continue

            import re

            m = re.search(r"(\d+)/(\d+)", content)
            if not m:
                continue

            mc = int(m.group(1))
            total = int(m.group(2))

            if total == num_items:
                if mc != prev_matched_count:
                    prev_matched_count = mc
                    matched_count = mc
                    stable_start = time.monotonic()
                    end_ns = time.perf_counter_ns()
                elif stable_start > 0:
                    if time.monotonic() - stable_start >= REQUIRED_STABLE_S:
                        completed = True
                        break

        if end_ns == 0:
            end_ns = time.perf_counter_ns()

        # Exit skim
        subprocess.run(
            ["tmux", "send-keys", "-t", session_name, "Escape"],
            capture_output=True,
        )
        time.sleep(0.1)

        # If perf is recording, wait for it to exit and flush data before we
        # kill the tmux session.  perf record is the parent of sk in the shell
        # pipeline, so it exits on its own once sk does – we just need to give
        # it enough time to finish writing.
        if perf_output and pane_pid:
            perf_wait_start = time.monotonic()
            while time.monotonic() - perf_wait_start < 15.0:
                result = subprocess.run(
                    ["pgrep", "-P", str(pane_pid), "-f", "perf record"],
                    capture_output=True,
                )
                if result.returncode != 0:
                    # perf has exited
                    break
                time.sleep(0.1)
            else:
                print(
                    "Warning: perf record did not exit within 15 s; perf data may be incomplete.",
                    file=sys.stderr,
                )

        # Wait for monitor
        if monitor is not None:
            monitor.join(timeout=2.0)

        elapsed_s = (end_ns - start_ns) / 1e9
        rate = num_items / elapsed_s if elapsed_s > 0 else 0

        peak_mem_kb = monitor.peak_mem_kb if monitor and monitor.peak_mem_kb else 0
        peak_cpu = monitor.peak_cpu if monitor and monitor.peak_cpu else 0.0

        recorded_perf = perf_output if perf_output else None

        return {
            "elapsed_s": elapsed_s,
            "rate": rate,
            "matched": matched_count,
            "peak_mem_kb": peak_mem_kb if peak_mem_kb else None,
            "peak_cpu": peak_cpu if peak_cpu else None,
            "completed": completed,
            "perf_file": recorded_perf,
        }

    finally:
        subprocess.run(
            ["tmux", "kill-session", "-t", session_name],
            capture_output=True,
        )
        try:
            os.unlink(status_file)
        except OSError:
            pass


# ---------------------------------------------------------------------------
# Aggregate statistics
# ---------------------------------------------------------------------------


def _avg(values):
    vals = [v for v in values if v is not None]
    return sum(vals) / len(vals) if vals else None


def _min(values):
    vals = [v for v in values if v is not None]
    return min(vals) if vals else None


def _max(values):
    vals = [v for v in values if v is not None]
    return max(vals) if vals else None


def aggregate(results: list) -> dict:
    completed_results = [r for r in results if r["completed"]]
    times = [r["elapsed_s"] for r in completed_results]
    rates = [r["rate"] for r in completed_results]
    matched = [r["matched"] for r in completed_results]
    mems = [r["peak_mem_kb"] for r in completed_results]
    cpus = [r["peak_cpu"] for r in completed_results]
    completed = len(completed_results)

    return {
        "completed": completed,
        "runs": len(results),
        "avg_time": _avg(times),
        "min_time": _min(times),
        "max_time": _max(times),
        "avg_rate": _avg(rates),
        "min_rate": _min(rates),
        "max_rate": _max(rates),
        "avg_matched": _avg(matched),
        "min_matched": _min(matched),
        "max_matched": _max(matched),
        "avg_mem": _avg(mems),
        "min_mem": _min(mems),
        "max_mem": _max(mems),
        "avg_cpu": _avg(cpus),
        "min_cpu": _min(cpus),
        "max_cpu": _max(cpus),
    }


# ---------------------------------------------------------------------------
# Formatting helpers
# ---------------------------------------------------------------------------


def _pct(baseline, value):
    """Return a +/-XX.X% string comparing *value* to *baseline*."""
    if baseline is None or value is None or baseline == 0:
        return ""
    diff = (value - baseline) / abs(baseline) * 100
    sign = "+" if diff >= 0 else ""
    return f"{sign}{diff:.1f}%"


def _fmt_mem(kb):
    if kb is None:
        return None
    return kb / 1024  # MB


def _fmt_optional(value, fmt):
    if value is None:
        return "N/A"
    return fmt.format(value)


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------


def print_human(
    binary_label: str,
    agg: dict,
    num_items: int,
    baseline: dict | None = None,
    is_baseline: bool = False,
):
    tag = " [baseline]" if is_baseline else ""
    print(f"\n=== Results: {binary_label}{tag} ===")
    print(f"Completed runs: {agg['completed']} / {agg['runs']}")

    def cmp(key, baseline_key=None):
        """Format comparison vs baseline for a given aggregate key."""
        bk = baseline_key or key
        if baseline is None or is_baseline:
            return ""
        return "  " + _pct(baseline.get(bk), agg.get(key))

    # Items matched
    avg_m = _fmt_optional(agg["avg_matched"], "{:.0f}")
    min_m = _fmt_optional(agg["min_matched"], "{:.0f}")
    max_m = _fmt_optional(agg["max_matched"], "{:.0f}")
    print(
        f"Average items matched: {avg_m} / {num_items}"
        f"  (min: {min_m}, max: {max_m})"
        f"{cmp('avg_matched')}"
    )

    # Time
    avg_t = _fmt_optional(agg["avg_time"], "{:.3f}s")
    min_t = _fmt_optional(agg["min_time"], "{:.3f}s")
    max_t = _fmt_optional(agg["max_time"], "{:.3f}s")
    # Lower time is better, so flip sign for display
    time_cmp = ""
    if (
        baseline
        and not is_baseline
        and baseline.get("avg_time")
        and agg.get("avg_time")
    ):
        diff = (
            (agg["avg_time"] - baseline["avg_time"]) / abs(baseline["avg_time"]) * 100
        )
        sign = "+" if diff >= 0 else ""
        time_cmp = f"  {sign}{diff:.1f}%"
    print(f"Average time: {avg_t}  (min: {min_t}, max: {max_t}){time_cmp}")

    # Rate
    avg_r = _fmt_optional(agg["avg_rate"], "{:.0f}")
    min_r = _fmt_optional(agg["min_rate"], "{:.0f}")
    max_r = _fmt_optional(agg["max_rate"], "{:.0f}")
    print(
        f"Average items/second: {avg_r}  (min: {min_r}, max: {max_r}){cmp('avg_rate')}"
    )

    # Memory
    if agg["avg_mem"] is not None:
        avg_mb = _fmt_mem(agg["avg_mem"])
        min_mb = _fmt_mem(agg["min_mem"])
        max_mb = _fmt_mem(agg["max_mem"])
        print(
            f"Average peak memory usage: {avg_mb:.1f} MB"
            f"  (min: {min_mb:.1f} MB, max: {max_mb:.1f} MB)"
            f"{cmp('avg_mem')}"
        )

    # CPU
    if agg["avg_cpu"] is not None:
        avg_c = _fmt_optional(agg["avg_cpu"], "{:.1f}%")
        min_c = _fmt_optional(agg["min_cpu"], "{:.1f}%")
        max_c = _fmt_optional(agg["max_cpu"], "{:.1f}%")
        print(
            f"Average peak CPU usage: {avg_c}"
            f"  (min: {min_c}, max: {max_c})"
            f"{cmp('avg_cpu')}"
        )


def print_json_multi(binaries: list, aggregates: list, num_items: int, runs: int):
    output = []
    for binary, agg in zip(binaries, aggregates):
        entry = {
            "binary": binary,
            "num_items": num_items,
            "runs": runs,
            "completed_runs": agg["completed"],
            "items_matched": {
                "avg": agg["avg_matched"],
                "min": agg["min_matched"],
                "max": agg["max_matched"],
            },
            "time_s": {
                "avg": agg["avg_time"],
                "min": agg["min_time"],
                "max": agg["max_time"],
            },
            "items_per_second": {
                "avg": agg["avg_rate"],
                "min": agg["min_rate"],
                "max": agg["max_rate"],
            },
            "peak_memory_kb": {
                "avg": agg["avg_mem"],
                "min": agg["min_mem"],
                "max": agg["max_mem"],
            },
            "peak_cpu": {
                "avg": agg["avg_cpu"],
                "min": agg["min_cpu"],
                "max": agg["max_cpu"],
            },
        }
        output.append(entry)
    print(json.dumps(output if len(output) > 1 else output[0]))


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def _perf_path_for(binary: str, explicit: str) -> str:
    """Return the perf output file path.

    If *explicit* is a non-empty string use it directly; otherwise build an
    auto-named path of the form ``perf-<basename>-<timestamp>.data`` in the
    current working directory.
    """
    if explicit:
        return explicit
    ts = int(time.time())
    base = os.path.basename(binary).replace(" ", "_") or "sk"
    return f"perf-{base}-{ts}.data"


def main():
    import re  # ensure import at top of main scope for run_once

    binaries, opts, extra_args = parse_args(sys.argv[1:])

    num_items = opts.num_items
    query = opts.query
    runs = opts.runs
    warmup = opts.warmup
    input_file = opts.file
    generate_file = opts.generate_file
    as_json = opts.json
    record_perf = opts.perf is not None  # True when -p/--perf was supplied
    perf_explicit = opts.perf or ""  # "" means auto-name

    # ---- generate-file mode ------------------------------------------------
    if generate_file:
        print(f"Generating {num_items} items to {generate_file}...", file=sys.stderr)
        generate_test_data(generate_file, num_items)
        print(f"Generated {num_items} items successfully", file=sys.stderr)
        return

    # ---- prepare input data ------------------------------------------------
    cleanup_input = False
    if input_file:
        if not os.path.isfile(input_file):
            print(f"Error: Input file '{input_file}' not found", file=sys.stderr)
            sys.exit(1)
        tmp_file = input_file
        with open(input_file) as fh:
            num_items = sum(1 for _ in fh)
        print(f"Using input file with {num_items} items", file=sys.stderr)
    else:
        fd, tmp_file = tempfile.mkstemp(prefix="skim_bench_input_")
        os.close(fd)
        cleanup_input = True
        print("Generating test data...", file=sys.stderr)
        generate_test_data(tmp_file, num_items)

    try:
        # ---- header ---------------------------------------------------------
        binary_list = ", ".join(binaries)
        print(f"=== Skim Ingestion + Matching Benchmark ===", file=sys.stderr)
        print(
            f"Binaries: {binary_list} | Items: {num_items} | "
            f"Query: '{query}' | Warmup: {warmup} | Runs: {runs} (per binary)",
            file=sys.stderr,
        )
        if input_file:
            print(f"Input file: {input_file}", file=sys.stderr)
        if extra_args:
            print(f"Extra args: {' '.join(extra_args)}", file=sys.stderr)
        if record_perf:
            print("Perf recording: enabled (final measured run only)", file=sys.stderr)

        # ---- warmup (results discarded) -------------------------------------
        if warmup > 0:
            print(f"\n=== Warmup ({warmup} run(s) per binary) ===", file=sys.stderr)
            for bi, binary in enumerate(binaries):
                for wu in range(1, warmup + 1):
                    print(
                        f"  Warmup {wu}/{warmup} — {binary} ...",
                        file=sys.stderr,
                    )
                    run_once(
                        binary_path=binary,
                        query=query,
                        tmp_file=tmp_file,
                        num_items=num_items,
                        extra_args=extra_args,
                        run_index=wu,
                        session_suffix=f"warmup_b{bi}",
                        perf_output=None,  # never record during warmup
                    )

        # ---- run benchmark in round-robin -----------------------------------
        # all_results[i] = list of per-run dicts for binaries[i]
        all_results = [[] for _ in binaries]

        # Determine which (run_num, bi) pairs get perf recording.
        # We record only on the very last run for each binary to avoid
        # overwriting data and to minimise measurement overhead.
        perf_files: dict[int, str] = {}  # bi -> path
        if record_perf:
            for bi, binary in enumerate(binaries):
                perf_files[bi] = _perf_path_for(binary, perf_explicit if len(binaries) == 1 else "")

        for run_num in range(1, runs + 1):
            for bi, binary in enumerate(binaries):
                label = f"[{os.path.basename(binary)}]"
                if runs > 1 or len(binaries) > 1:
                    print(
                        f"\n=== Run {run_num}/{runs} — binary {bi + 1}/{len(binaries)}: {binary} ===",
                        file=sys.stderr,
                    )

                # Attach perf only on the final run for this binary
                this_perf = perf_files.get(bi) if run_num == runs else None

                result = run_once(
                    binary_path=binary,
                    query=query,
                    tmp_file=tmp_file,
                    num_items=num_items,
                    extra_args=extra_args,
                    run_index=run_num,
                    session_suffix=f"b{bi}",
                    perf_output=this_perf,
                )
                all_results[bi].append(result)

                if runs > 1 or len(binaries) > 1:
                    status = "COMPLETED" if result["completed"] else "TIMEOUT"
                    print(f"Status: {status}", file=sys.stderr)
                    print(
                        f"Items matched: {result['matched']} / {num_items}",
                        file=sys.stderr,
                    )
                    print(f"Total time: {result['elapsed_s']:.3f}s", file=sys.stderr)
                    print(f"Items/second: {result['rate']:.0f}", file=sys.stderr)
                    if result["peak_mem_kb"]:
                        print(
                            f"Peak memory usage: {result['peak_mem_kb'] / 1024:.1f} MB",
                            file=sys.stderr,
                        )
                    if result["peak_cpu"]:
                        print(
                            f"Peak CPU usage: {result['peak_cpu']:.1f}%",
                            file=sys.stderr,
                        )
                    if result.get("perf_file"):
                        print(
                            f"Perf data: {result['perf_file']}",
                            file=sys.stderr,
                        )

        # ---- aggregate ------------------------------------------------------
        aggregates = [aggregate(all_results[i]) for i in range(len(binaries))]

        # ---- output ---------------------------------------------------------
        if as_json:
            print_json_multi(binaries, aggregates, num_items, runs)
        else:
            baseline_agg = aggregates[0]
            for i, (binary, agg) in enumerate(zip(binaries, aggregates)):
                print_human(
                    binary_label=binary,
                    agg=agg,
                    num_items=num_items,
                    baseline=baseline_agg if len(binaries) > 1 else None,
                    is_baseline=(i == 0),
                )

            # Summary comparison table when multiple binaries
            if len(binaries) > 1:
                print(f"\n=== Comparison Summary (vs baseline: {binaries[0]}) ===")
                header = (
                    f"{'Binary':<40} {'Avg time':>12} {'Δ time':>10}"
                    f" {'Avg rate':>14} {'Δ rate':>10}"
                    f" {'Avg mem (MB)':>14} {'Δ mem':>10}"
                    f" {'Avg CPU (%)':>12} {'Δ CPU':>10}"
                )
                print(header)
                print("-" * len(header))
                for i, (binary, agg) in enumerate(zip(binaries, aggregates)):
                    t = (
                        f"{agg['avg_time']:.3f}s"
                        if agg["avg_time"] is not None
                        else "N/A"
                    )
                    r = (
                        f"{agg['avg_rate']:.0f}"
                        if agg["avg_rate"] is not None
                        else "N/A"
                    )
                    m = (
                        f"{agg['avg_mem'] / 1024:.1f}"
                        if agg["avg_mem"] is not None
                        else "N/A"
                    )
                    c = (
                        f"{agg['avg_cpu']:.1f}"
                        if agg["avg_cpu"] is not None
                        else "N/A"
                    )
                    if i == 0:
                        dt = "baseline"
                        dr = "baseline"
                        dm = "baseline"
                        dc = "baseline"
                    else:
                        dt = _pct(baseline_agg["avg_time"], agg["avg_time"])
                        dr = _pct(baseline_agg["avg_rate"], agg["avg_rate"])
                        dm = _pct(baseline_agg["avg_mem"], agg["avg_mem"])
                        dc = _pct(baseline_agg["avg_cpu"], agg["avg_cpu"])
                    name = os.path.basename(binary) if len(binary) > 40 else binary
                    print(
                        f"{name:<40} {t:>12} {dt:>10}"
                        f" {r:>14} {dr:>10}"
                        f" {m:>14} {dm:>10}"
                        f" {c:>12} {dc:>10}"
                    )

        # ---- perf summary ---------------------------------------------------
        if record_perf:
            print("\n=== Perf recording output ===", file=sys.stderr)
            for bi, binary in enumerate(binaries):
                path = perf_files.get(bi, "")
                if path and os.path.isfile(path):
                    print(f"  [{binary}] perf data: {path}", file=sys.stderr)
                else:
                    print(
                        f"  [{binary}] perf data not found (perf may have failed)",
                        file=sys.stderr,
                    )

    finally:
        if cleanup_input:
            try:
                os.unlink(tmp_file)
            except OSError:
                pass


if __name__ == "__main__":
    main()

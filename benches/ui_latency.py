#!/usr/bin/env python3
"""UI latency under streaming input (python3 stdlib only, Linux/macOS).

Runs `sk` in a PTY, streams a fixture into its stdin at a fixed rate, sends
mouse-wheel events and measures, from sk's trace log:

  handled  wheel event written -> `App::handle_mouse` runs
  e2e      wheel event written -> end of the first render after it
  peak_rss VmHWM of sk at the end of the run (Linux)

The end of a render is the `render done` trace line when sk logs it, else the
start of the next handled event (an over-estimate of up to one heartbeat).

Example:
  python3 benches/ui_latency.py --bin target/release/sk --query a --rate 500000
"""

import argparse
import datetime
import os
import pty
import re
import select
import signal
import statistics
import struct
import tempfile
import termios
import fcntl
import threading
import time

WHEEL_DOWN = b"\x1b[<65;20;10M"
EVENT_RE = re.compile(r"handling event ([A-Za-z]+)")


def parse_ts(line):
    stamp = line[1:].split(" ", 1)[0].rstrip("Z")
    base, frac = stamp.split(".")
    return datetime.datetime.fromisoformat(base).replace(tzinfo=datetime.timezone.utc).timestamp() + int(frac) / 10 ** len(frac)


def feed(fd, path, rate, stop):
    """Write `rate` lines per second to `fd`, in 10 ms slices."""
    per_slice = max(1, rate // 100)
    deadline = time.time()
    with open(path, "rb") as src:
        while not stop.is_set():
            lines = [line for line in (src.readline() for _ in range(per_slice)) if line]
            if not lines:
                break
            try:
                os.write(fd, b"".join(lines))
            except OSError:  # sk exited
                break
            deadline += 0.01
            delay = deadline - time.time()
            if delay > 0:
                time.sleep(delay)
    os.close(fd)


def run_once(args, log):
    read_end, write_end = os.pipe()
    pid, master = pty.fork()
    if pid == 0:
        os.close(write_end)
        os.dup2(read_end, 0)
        devnull = os.open(os.devnull, os.O_WRONLY)
        os.dup2(devnull, 1)
        env = {k: v for k, v in os.environ.items() if not k.startswith("SKIM_")}
        env.update(SKIM_LOG="skim::tui::app=trace", SKIM_LOG_FILE=log, TERM="xterm-256color")
        argv = [args.bin] + (["--query", args.query] if args.query else []) + args.extra
        os.execve(args.bin, argv, env)
    os.close(read_end)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", args.rows, args.cols, 0, 0))

    stop = threading.Event()
    feeder = threading.Thread(target=feed, args=(write_end, args.input, args.rate, stop), daemon=True)
    feeder.start()

    sends = []
    next_send = time.time() + args.warmup
    end = next_send + args.events * args.interval + 1.0
    while time.time() < end:
        if len(sends) < args.events and time.time() >= next_send:
            os.write(master, WHEEL_DOWN)
            sends.append(time.time())
            next_send += args.interval
        ready, _, _ = select.select([master], [], [], 0.005)
        if ready:
            try:
                os.read(master, 1 << 16)  # drain, else sk blocks on its output
            except OSError:
                break
    stop.set()
    peak_kb = None
    try:  # Linux only
        with open(f"/proc/{pid}/status") as status:
            peak_kb = next((int(l.split()[1]) for l in status if l.startswith("VmHWM:")), None)
    except OSError:
        pass
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    os.close(master)
    return sends, peak_kb


def analyse(sends, log):
    events = []  # (time, kind)
    render_done = []
    with open(log, errors="replace") as f:
        for line in f:
            if "render done" in line:
                render_done.append(parse_ts(line))
            match = EVENT_RE.search(line)
            if match:
                events.append((parse_ts(line), match.group(1)))
    mouse = [t for t, kind in events if kind == "Mouse"]
    renders = [i for i, (_, kind) in enumerate(events) if kind == "Render"]

    def render_end(start_index):
        start = events[start_index][0]
        done = next((t for t in render_done if t >= start), None)
        if done is not None:
            return done
        return events[start_index + 1][0] if start_index + 1 < len(events) else None

    handled, e2e = [], []
    for sent, got in zip(sends, mouse):
        handled.append((got - sent) * 1000)
        first = next((i for i in renders if events[i][0] >= got), None)
        end = render_end(first) if first is not None else None
        if end is not None:
            e2e.append((end - sent) * 1000)
    durations = []
    for i in renders:
        end = render_end(i)
        if end is not None:
            durations.append((end - events[i][0]) * 1000)
    return handled, e2e, durations, len(sends) - len(mouse)


def summary(name, values):
    if not values:
        return f"{name:8} n=0"
    values = sorted(values)
    p90 = values[max(0, int(len(values) * 0.9) - 1)]
    return f"{name:8} n={len(values):4} p50={statistics.median(values):7.1f} p90={p90:7.1f} max={values[-1]:7.1f} ms"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bin", default="target/release/sk")
    parser.add_argument("--input", default="benches/fixtures/10M.txt")
    parser.add_argument("--query", default="a")
    parser.add_argument("--rate", type=int, default=500_000, help="lines per second")
    parser.add_argument("--events", type=int, default=120)
    parser.add_argument("--interval", type=float, default=0.1, help="seconds between wheel events")
    parser.add_argument("--warmup", type=float, default=0.3)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--rows", type=int, default=40)
    parser.add_argument("--cols", type=int, default=120)
    parser.add_argument("extra", nargs="*", help="extra sk arguments (after --)")
    args = parser.parse_args()
    args.bin = os.path.abspath(args.bin)

    handled, e2e, durations, lost, peaks = [], [], [], 0, []
    for run in range(args.repeat):
        with tempfile.NamedTemporaryFile(suffix=".log") as log:
            sends, peak_kb = run_once(args, log.name)
            h, e, d, l = analyse(sends, log.name)
        peak = f" peak_rss={peak_kb // 1024} MiB" if peak_kb else ""
        print(f"run {run + 1}: {summary('e2e', e)}{peak}")
        peaks += [peak_kb] if peak_kb else []
        handled += h
        e2e += e
        durations += d
        lost += l
    print(f"{args.bin} query={args.query!r} rate={args.rate}/s")
    print(summary("handled", handled))
    print(summary("e2e", e2e))
    print(summary("render", durations))
    if peaks:
        print(f"peak_rss max={max(peaks) // 1024} MiB")
    if lost:
        print(f"warning: {lost} wheel events were not handled before the end of the run")


if __name__ == "__main__":
    main()

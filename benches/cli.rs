//! Interactive benchmark binary for skim.
//!
//! Measures ingestion + matching rate by running sk (or any compatible binary)
//! inside a tmux session, streaming generated (or pre-existing) test data into
//! it, and polling the status line until the matched count stabilises.
//!
//! Binary names are resolved to absolute paths via `which` before use, so bare
//! names like `sk` or `fzf` work as long as they are on `$PATH`.
//!
//! Invoke via the `bench-cli` cargo alias:
//!
//! ```text
//! cargo bench-cli                                      # defaults
//! cargo bench-cli -- sk -n 500000 -q foo
//! cargo bench-cli -- ./old/sk ./new/sk -r 5           # compare two binaries
//! cargo bench-cli -- sk -r 5                          # 5 runs, show average
//! cargo bench-cli -- sk -f input.txt -q search        # use existing file
//! cargo bench-cli -- -g testdata.txt -n 2000000       # generate file and exit
//! cargo bench-cli -- sk -p                            # record perf (auto-named)
//! cargo bench-cli -- sk -p perf.data                 # record perf to perf.data
//! cargo bench-cli -- sk -j                            # JSON output
//! cargo bench-cli -- sk -r 3 -- --tiebreak=index     # pass extra flags to sk
//! ```

use clap::Parser;
use rand::RngExt as _;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_BINARY: &str = "./target/release/sk";
const DEFAULT_NUM_ITEMS: u64 = 1_000_000;
const DEFAULT_QUERY: &str = "test";

/// Seconds the matched count must be unchanged before declaring completion.
const REQUIRED_STABLE_S: f64 = 5.0;
/// Hard timeout per run.
const MAX_WAIT_S: f64 = 60.0;
/// Polling interval in milliseconds.
const CHECK_INTERVAL_MS: u64 = 50;

const WORDS: &[&str] = &[
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
];

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "bench",
    about = "Benchmark skim ingestion + matching rate in interactive mode",
    long_about = "Measures how fast skim can ingest items and display matched results \
                  by running sk inside a tmux session and polling the status line.",
    ignore_errors(true)
)]
struct Args {
    /// One or more paths to binaries (default: ./target/release/sk).
    /// When multiple are given they run in round-robin and the first is used
    /// as the baseline for +/- comparisons.
    #[arg(value_name = "BINARY_PATH")]
    binaries: Vec<String>,

    /// Number of items to generate
    #[arg(short = 'n', long, default_value_t = DEFAULT_NUM_ITEMS, value_name = "NUM")]
    num_items: u64,

    /// Query string to search
    #[arg(short = 'q', long, default_value = DEFAULT_QUERY)]
    query: String,

    /// Number of benchmark runs per binary
    #[arg(short = 'r', long, default_value_t = 1u32, value_name = "RUNS")]
    runs: u32,

    /// Number of warmup runs per binary
    #[arg(short = 'w', long, default_value_t = 1u32, value_name = "N")]
    warmup: u32,

    /// Use existing file as input instead of generating
    #[arg(short = 'f', long, value_name = "FILE")]
    file: Option<String>,

    /// Generate test data to file and exit
    #[arg(short = 'g', long, value_name = "FILE")]
    generate_file: Option<String>,

    /// Output results as JSON
    #[arg(short = 'j', long)]
    json: bool,

    /// Record perf data for the final benchmark run.
    /// Optionally specify the output file (default: auto-named
    /// perf-<binary>-<timestamp>.data).
    #[arg(
        short = 'p',
        long,
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "FILE"
    )]
    perf: Option<String>,

    /// Pass remaining arguments to the benchmarked binary
    #[arg(last = true)]
    extra_args: Vec<String>,
}

// ---------------------------------------------------------------------------
// Test-data generation
// ---------------------------------------------------------------------------

fn generate_test_data(output_file: &str, num_items: u64) -> std::io::Result<()> {
    let file = File::create(output_file)?;
    let mut writer = BufWriter::new(file);
    let mut rng = rand::rng();
    for i in 1..=num_items {
        let depth = rng.random_range(2..=10usize);
        let parts: Vec<&str> = (0..depth).map(|_| WORDS[rng.random_range(0..WORDS.len())]).collect();
        writeln!(writer, "{}_{}", parts.join("/"), i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Resource monitor
// ---------------------------------------------------------------------------

struct ResourcePeak {
    peak_mem_kb: u64,
    peak_cpu: f64,
}

struct ResourceMonitor {
    stats: Arc<Mutex<ResourcePeak>>,
    handle: thread::JoinHandle<()>,
}

impl ResourceMonitor {
    fn start(pid: u32) -> Self {
        let stats = Arc::new(Mutex::new(ResourcePeak {
            peak_mem_kb: 0,
            peak_cpu: 0.0,
        }));
        let stats_clone = Arc::clone(&stats);
        let handle = thread::spawn(move || {
            loop {
                match Command::new("ps")
                    .args(["-p", &pid.to_string(), "-o", "rss=,%cpu="])
                    .output()
                {
                    Ok(o) => {
                        let text = String::from_utf8_lossy(&o.stdout);
                        let line = text.trim();
                        if line.is_empty() {
                            break;
                        }
                        let mut parts = line.split_whitespace();
                        if let (Some(rss), Some(cpu)) = (parts.next(), parts.next()) {
                            if let (Ok(mem), Ok(cpu)) = (rss.parse::<u64>(), cpu.parse::<f64>()) {
                                let mut s = stats_clone.lock().unwrap();
                                s.peak_mem_kb = s.peak_mem_kb.max(mem);
                                s.peak_cpu = s.peak_cpu.max(cpu);
                            }
                        }
                    }
                    Err(_) => break,
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        ResourceMonitor { stats, handle }
    }

    fn join(self) -> (Option<u64>, Option<f64>) {
        let _ = self.handle.join();
        let s = self.stats.lock().unwrap();
        let mem = if s.peak_mem_kb > 0 { Some(s.peak_mem_kb) } else { None };
        let cpu = if s.peak_cpu > 0.0 { Some(s.peak_cpu) } else { None };
        (mem, cpu)
    }
}

// ---------------------------------------------------------------------------
// Single run
// ---------------------------------------------------------------------------

struct RunResult {
    elapsed_s: f64,
    rate: f64,
    matched: u64,
    peak_mem_kb: Option<u64>,
    peak_cpu: Option<f64>,
    completed: bool,
    perf_file: Option<String>,
}

/// Try for up to 5 s to find the sk child PID under `pane_pid`.
fn find_sk_pid(pane_pid: u32, binary_path: &str) -> u32 {
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if let Ok(o) = Command::new("pgrep")
            .args(["-P", &pane_pid.to_string(), "-f", binary_path])
            .output()
        {
            let text = String::from_utf8_lossy(&o.stdout);
            if let Some(first) = text.trim().lines().next() {
                if let Ok(pid) = first.trim().parse::<u32>() {
                    return pid;
                }
            }
        }
    }
    0
}

/// Return true if the process with the given PID is still alive.
fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

#[allow(clippy::too_many_arguments)]
fn run_once(
    binary_path: &str,
    query: &str,
    tmp_file: &str,
    num_items: u64,
    extra_args: &[String],
    run_index: u32,
    session_suffix: &str,
    perf_output: Option<&str>,
) -> RunResult {
    let session_name = format!("skim_bench_{}_{}_{}", std::process::id(), session_suffix, run_index);

    // Temp file for tmux save-buffer output
    let status_tmp = NamedTempFile::new().expect("failed to create status temp file");
    let status_path = status_tmp.path().to_string_lossy().into_owned();

    // Build environment (strip skim/fzf env vars)
    let env_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k != "HISTFILE" && k != "FZF_DEFAULT_OPTS" && k != "SKIM_DEFAULT_OPTIONS")
        .chain([("SHELL".into(), "/bin/sh".into())])
        .collect();

    // Create detached tmux session
    let _ = Command::new("tmux")
        .args(["new-session", "-s", &session_name, "-d"])
        .env_clear()
        .envs(env_vars)
        .output();

    for cmd in ["unset HISTFILE", "unset FZF_DEFAULT_OPTS", "unset SKIM_DEFAULT_OPTIONS"] {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &session_name, cmd, "Enter"])
            .output();
    }
    thread::sleep(Duration::from_millis(100));

    // Build the command to run inside the tmux pane
    let extra_str = extra_args.join(" ");
    let perf_prefix = match perf_output {
        Some(path) => format!("perf record -o {} -- ", path),
        None => String::new(),
    };
    let cmd_str = format!(
        "cat {} | {}{} --query '{}' {}",
        tmp_file, perf_prefix, binary_path, query, extra_str
    );

    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &session_name, &cmd_str, "Enter"])
        .output();

    let start = Instant::now();

    // Find the pane PID, then the sk child PID
    let pane_pid: u32 = Command::new("tmux")
        .args(["list-panes", "-t", &session_name, "-F", "#{pane_pid}"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .lines()
                .next()
                .and_then(|s| s.trim().parse().ok())
        })
        .unwrap_or(0);

    let sk_pid = if pane_pid > 0 {
        find_sk_pid(pane_pid, binary_path)
    } else {
        0
    };
    let monitor = if sk_pid > 0 {
        Some(ResourceMonitor::start(sk_pid))
    } else {
        None
    };

    // Poll the tmux pane until the matched count stabilises
    let re = regex::Regex::new(r"(\d+)/(\d+)").expect("valid regex");
    let mut completed = false;
    let mut matched_count: u64 = 0;
    let mut prev_matched: u64 = u64::MAX;
    let mut stable_since: Option<Instant> = None;
    let mut last_change_elapsed: Option<Duration> = None;
    let loop_start = Instant::now();
    let buf_name = format!("status-{}", session_name);

    loop {
        thread::sleep(Duration::from_millis(CHECK_INTERVAL_MS));

        if loop_start.elapsed().as_secs_f64() >= MAX_WAIT_S {
            break;
        }

        if sk_pid > 0 && !process_alive(sk_pid) {
            break;
        }

        let _ = Command::new("tmux")
            .args(["capture-pane", "-b", &buf_name, "-t", &session_name])
            .output();
        let _ = Command::new("tmux")
            .args(["save-buffer", "-b", &buf_name, &status_path])
            .output();

        let content = match fs::read_to_string(&status_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(caps) = re.captures(&content) {
            let mc: u64 = caps[1].parse().unwrap_or(0);
            let total: u64 = caps[2].parse().unwrap_or(0);

            if total == num_items {
                if mc != prev_matched {
                    prev_matched = mc;
                    matched_count = mc;
                    stable_since = Some(Instant::now());
                    last_change_elapsed = Some(start.elapsed());
                } else if stable_since.is_some_and(|t| t.elapsed().as_secs_f64() >= REQUIRED_STABLE_S) {
                    completed = true;
                    break;
                }
            }
        }
    }

    let elapsed_s = last_change_elapsed.unwrap_or_else(|| start.elapsed()).as_secs_f64();

    // Send Escape to exit sk
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &session_name, "Escape"])
        .output();
    thread::sleep(Duration::from_millis(100));

    // Wait for perf record to finish writing before killing the session
    if perf_output.is_some() && pane_pid > 0 {
        let perf_wait = Instant::now();
        loop {
            if perf_wait.elapsed().as_secs_f64() >= 15.0 {
                eprintln!("Warning: perf record did not exit within 15 s; perf data may be incomplete.");
                break;
            }
            let still_running = Command::new("pgrep")
                .args(["-P", &pane_pid.to_string(), "-f", "perf record"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !still_running {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    let (peak_mem_kb, peak_cpu) = monitor.map(ResourceMonitor::join).unwrap_or((None, None));

    let rate = if elapsed_s > 0.0 {
        num_items as f64 / elapsed_s
    } else {
        0.0
    };

    let _ = Command::new("tmux")
        .args(["kill-session", "-t", &session_name])
        .output();

    RunResult {
        elapsed_s,
        rate,
        matched: matched_count,
        peak_mem_kb,
        peak_cpu,
        completed,
        perf_file: perf_output.map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// Aggregate statistics
// ---------------------------------------------------------------------------

struct AggResult {
    completed: usize,
    runs: usize,
    avg_time: Option<f64>,
    min_time: Option<f64>,
    max_time: Option<f64>,
    avg_rate: Option<f64>,
    min_rate: Option<f64>,
    max_rate: Option<f64>,
    avg_matched: Option<f64>,
    min_matched: Option<f64>,
    max_matched: Option<f64>,
    avg_mem: Option<f64>,
    min_mem: Option<f64>,
    max_mem: Option<f64>,
    avg_cpu: Option<f64>,
    min_cpu: Option<f64>,
    max_cpu: Option<f64>,
}

fn avg(vals: &[f64]) -> Option<f64> {
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

fn aggregate(results: &[RunResult]) -> AggResult {
    let done: Vec<&RunResult> = results.iter().filter(|r| r.completed).collect();

    let times: Vec<f64> = done.iter().map(|r| r.elapsed_s).collect();
    let rates: Vec<f64> = done.iter().map(|r| r.rate).collect();
    let matched: Vec<f64> = done.iter().map(|r| r.matched as f64).collect();
    let mems: Vec<f64> = done.iter().filter_map(|r| r.peak_mem_kb.map(|v| v as f64)).collect();
    let cpus: Vec<f64> = done.iter().filter_map(|r| r.peak_cpu).collect();

    AggResult {
        completed: done.len(),
        runs: results.len(),
        avg_time: avg(&times),
        min_time: times.iter().copied().reduce(f64::min),
        max_time: times.iter().copied().reduce(f64::max),
        avg_rate: avg(&rates),
        min_rate: rates.iter().copied().reduce(f64::min),
        max_rate: rates.iter().copied().reduce(f64::max),
        avg_matched: avg(&matched),
        min_matched: matched.iter().copied().reduce(f64::min),
        max_matched: matched.iter().copied().reduce(f64::max),
        avg_mem: avg(&mems),
        min_mem: mems.iter().copied().reduce(f64::min),
        max_mem: mems.iter().copied().reduce(f64::max),
        avg_cpu: avg(&cpus),
        min_cpu: cpus.iter().copied().reduce(f64::min),
        max_cpu: cpus.iter().copied().reduce(f64::max),
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn pct(baseline: Option<f64>, value: Option<f64>) -> String {
    match (baseline, value) {
        (Some(b), Some(v)) if b != 0.0 => {
            let diff = (v - b) / b.abs() * 100.0;
            if diff >= 0.0 {
                format!("+{:.1}%", diff)
            } else {
                format!("{:.1}%", diff)
            }
        }
        _ => String::new(),
    }
}

fn fmt_opt(value: Option<f64>, fmt: impl Fn(f64) -> String) -> String {
    value.map(fmt).unwrap_or_else(|| "N/A".into())
}

// ---------------------------------------------------------------------------
// Human-readable output
// ---------------------------------------------------------------------------

fn print_human(binary_label: &str, agg: &AggResult, num_items: u64, baseline: Option<&AggResult>, is_baseline: bool) {
    let tag = if is_baseline { " [baseline]" } else { "" };
    println!("\n=== Results: {}{} ===", binary_label, tag);
    println!("Completed runs: {} / {}", agg.completed, agg.runs);

    // Comparison helper: empty when there's no baseline or this IS the baseline
    let cmp = |val: Option<f64>, base_val: Option<f64>| -> String {
        if baseline.is_none() || is_baseline {
            String::new()
        } else {
            format!("  {}", pct(base_val, val))
        }
    };

    // Matched
    println!(
        "Average items matched: {}  (min: {}, max: {}){num_items_total}",
        fmt_opt(agg.avg_matched, |v| format!("{:.0}", v)),
        fmt_opt(agg.min_matched, |v| format!("{:.0}", v)),
        fmt_opt(agg.max_matched, |v| format!("{:.0}", v)),
        num_items_total = format!(
            " / {}{}",
            num_items,
            cmp(agg.avg_matched, baseline.and_then(|b| b.avg_matched))
        ),
    );

    // Time (lower is better — preserve Python's sign convention: positive means slower)
    let time_cmp = if let (Some(b), Some(v), false) = (baseline.and_then(|b| b.avg_time), agg.avg_time, is_baseline) {
        let diff = (v - b) / b.abs() * 100.0;
        if diff >= 0.0 {
            format!("  +{:.1}%", diff)
        } else {
            format!("  {:.1}%", diff)
        }
    } else {
        String::new()
    };
    println!(
        "Average time: {}  (min: {}, max: {}){}",
        fmt_opt(agg.avg_time, |v| format!("{:.3}s", v)),
        fmt_opt(agg.min_time, |v| format!("{:.3}s", v)),
        fmt_opt(agg.max_time, |v| format!("{:.3}s", v)),
        time_cmp,
    );

    // Rate
    println!(
        "Average items/second: {}  (min: {}, max: {}){}",
        fmt_opt(agg.avg_rate, |v| format!("{:.0}", v)),
        fmt_opt(agg.min_rate, |v| format!("{:.0}", v)),
        fmt_opt(agg.max_rate, |v| format!("{:.0}", v)),
        cmp(agg.avg_rate, baseline.and_then(|b| b.avg_rate)),
    );

    // Memory (optional)
    if agg.avg_mem.is_some() {
        let mb = |kb: Option<f64>| fmt_opt(kb, |v| format!("{:.1} MB", v / 1024.0));
        println!(
            "Average peak memory usage: {}  (min: {}, max: {}){}",
            mb(agg.avg_mem),
            mb(agg.min_mem),
            mb(agg.max_mem),
            cmp(agg.avg_mem, baseline.and_then(|b| b.avg_mem)),
        );
    }

    // CPU (optional)
    if agg.avg_cpu.is_some() {
        println!(
            "Average peak CPU usage: {}  (min: {}, max: {}){}",
            fmt_opt(agg.avg_cpu, |v| format!("{:.1}%", v)),
            fmt_opt(agg.min_cpu, |v| format!("{:.1}%", v)),
            fmt_opt(agg.max_cpu, |v| format!("{:.1}%", v)),
            cmp(agg.avg_cpu, baseline.and_then(|b| b.avg_cpu)),
        );
    }
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonMinMaxAvg {
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Serialize)]
struct JsonEntry {
    binary: String,
    num_items: u64,
    runs: u32,
    completed_runs: usize,
    items_matched: JsonMinMaxAvg,
    time_s: JsonMinMaxAvg,
    items_per_second: JsonMinMaxAvg,
    peak_memory_kb: JsonMinMaxAvg,
    peak_cpu: JsonMinMaxAvg,
}

fn build_json_entry(binary: &str, agg: &AggResult, num_items: u64, runs: u32) -> JsonEntry {
    JsonEntry {
        binary: binary.to_owned(),
        num_items,
        runs,
        completed_runs: agg.completed,
        items_matched: JsonMinMaxAvg {
            avg: agg.avg_matched,
            min: agg.min_matched,
            max: agg.max_matched,
        },
        time_s: JsonMinMaxAvg {
            avg: agg.avg_time,
            min: agg.min_time,
            max: agg.max_time,
        },
        items_per_second: JsonMinMaxAvg {
            avg: agg.avg_rate,
            min: agg.min_rate,
            max: agg.max_rate,
        },
        peak_memory_kb: JsonMinMaxAvg {
            avg: agg.avg_mem,
            min: agg.min_mem,
            max: agg.max_mem,
        },
        peak_cpu: JsonMinMaxAvg {
            avg: agg.avg_cpu,
            min: agg.min_cpu,
            max: agg.max_cpu,
        },
    }
}

fn print_json(binaries: &[String], aggregates: &[AggResult], num_items: u64, runs: u32) {
    let entries: Vec<JsonEntry> = binaries
        .iter()
        .zip(aggregates)
        .map(|(b, a)| build_json_entry(b, a, num_items, runs))
        .collect();
    if entries.len() == 1 {
        println!("{}", serde_json::to_string(&entries[0]).unwrap());
    } else {
        println!("{}", serde_json::to_string(&entries).unwrap());
    }
}

// ---------------------------------------------------------------------------
// Perf file path helper
// ---------------------------------------------------------------------------

fn perf_path_for(binary: &str, explicit: &str) -> String {
    if !explicit.is_empty() {
        return explicit.to_owned();
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let base = Path::new(binary)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.replace(' ', "_"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "sk".into());
    format!("perf-{}-{}.data", base, ts)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // `cargo bench` injects `--bench` into argv for harness=false benches;
    // strip it before clap sees it so it doesn't land in `extra_args` or
    // cause an "unexpected argument" error.
    let raw: Vec<String> = std::env::args().filter(|a| a != "--bench").collect();
    let mut args = Args::parse_from(raw);

    if args.binaries.is_empty() {
        args.binaries.push(DEFAULT_BINARY.to_owned());
    }

    // Resolve every binary to an absolute path, the same as replacing
    // `<name>` with `$(which <name>)` at the call site.
    for binary in &mut args.binaries {
        match which::which(&*binary) {
            Ok(resolved) => *binary = resolved.to_string_lossy().into_owned(),
            Err(e) => {
                eprintln!("Error: cannot resolve binary '{}': {}", binary, e);
                std::process::exit(1);
            }
        }
    }

    if args.file.is_some() && args.generate_file.is_some() {
        eprintln!("error: cannot use both --file and --generate-file");
        std::process::exit(1);
    }

    // ---- generate-file mode -----------------------------------------------
    if let Some(ref path) = args.generate_file {
        eprintln!("Generating {} items to {} ...", args.num_items, path);
        generate_test_data(path, args.num_items).expect("failed to write test data");
        eprintln!("Generated {} items successfully", args.num_items);
        return;
    }

    // ---- prepare input data -----------------------------------------------
    let (tmp_file_path, _tmp_file_handle, num_items) = if let Some(ref path) = args.file {
        if !Path::new(path).is_file() {
            eprintln!("Error: Input file '{}' not found", path);
            std::process::exit(1);
        }
        let count = fs::read_to_string(path)
            .expect("failed to read input file")
            .lines()
            .count() as u64;
        eprintln!("Using input file with {} items", count);
        (path.clone(), None::<NamedTempFile>, count)
    } else {
        let tmp = NamedTempFile::new().expect("failed to create temp input file");
        let path = tmp.path().to_string_lossy().into_owned();
        eprintln!("Generating test data...");
        generate_test_data(&path, args.num_items).expect("failed to generate test data");
        (path, Some(tmp), args.num_items)
    };

    let binaries = &args.binaries;
    let query = &args.query;
    let runs = args.runs;
    let warmup = args.warmup;
    let extra_args = &args.extra_args;
    let record_perf = args.perf.is_some();
    let perf_explicit = args.perf.as_deref().unwrap_or("");

    // ---- header ------------------------------------------------------------
    eprintln!("=== Skim Ingestion + Matching Benchmark ===");
    eprintln!(
        "Binaries: {} | Items: {} | Query: '{}' | Warmup: {} | Runs: {} (per binary)",
        binaries.join(", "),
        num_items,
        query,
        warmup,
        runs,
    );
    if args.file.is_some() {
        eprintln!("Input file: {}", tmp_file_path);
    }
    if !extra_args.is_empty() {
        eprintln!("Extra args: {}", extra_args.join(" "));
    }
    if record_perf {
        eprintln!("Perf recording: enabled (final measured run only)");
    }

    // ---- warmup (results discarded) ----------------------------------------
    if warmup > 0 {
        eprintln!("\n=== Warmup ({} run(s) per binary) ===", warmup);
        for (bi, binary) in binaries.iter().enumerate() {
            for wu in 1..=warmup {
                eprintln!("  Warmup {}/{} — {} ...", wu, warmup, binary);
                run_once(
                    binary,
                    query,
                    &tmp_file_path,
                    num_items,
                    extra_args,
                    wu,
                    &format!("warmup_b{}", bi),
                    None,
                );
            }
        }
    }

    // ---- measured runs in round-robin ---------------------------------------
    let mut all_results: Vec<Vec<RunResult>> = (0..binaries.len()).map(|_| Vec::new()).collect();

    // Determine perf output paths (one per binary, recorded only on last run)
    let perf_files: Vec<Option<String>> = if record_perf {
        binaries
            .iter()
            .enumerate()
            .map(|(bi, binary)| {
                let explicit = if binaries.len() == 1 { perf_explicit } else { "" };
                let _ = bi;
                Some(perf_path_for(binary, explicit))
            })
            .collect()
    } else {
        vec![None; binaries.len()]
    };

    for run_num in 1..=runs {
        for (bi, binary) in binaries.iter().enumerate() {
            if runs > 1 || binaries.len() > 1 {
                eprintln!(
                    "\n=== Run {}/{} — binary {}/{}: {} ===",
                    run_num,
                    runs,
                    bi + 1,
                    binaries.len(),
                    binary
                );
            }

            // Attach perf only on the final run for this binary
            let this_perf = if run_num == runs {
                perf_files[bi].as_deref()
            } else {
                None
            };

            let result = run_once(
                binary,
                query,
                &tmp_file_path,
                num_items,
                extra_args,
                run_num,
                &format!("b{}", bi),
                this_perf,
            );

            if runs > 1 || binaries.len() > 1 {
                eprintln!("Status: {}", if result.completed { "COMPLETED" } else { "TIMEOUT" });
                eprintln!("Items matched: {} / {}", result.matched, num_items);
                eprintln!("Total time: {:.3}s", result.elapsed_s);
                eprintln!("Items/second: {:.0}", result.rate);
                if let Some(kb) = result.peak_mem_kb {
                    eprintln!("Peak memory usage: {:.1} MB", kb as f64 / 1024.0);
                }
                if let Some(cpu) = result.peak_cpu {
                    eprintln!("Peak CPU usage: {:.1}%", cpu);
                }
                if let Some(ref pf) = result.perf_file {
                    eprintln!("Perf data: {}", pf);
                }
            }

            all_results[bi].push(result);
        }
    }

    // ---- aggregate ---------------------------------------------------------
    let aggregates: Vec<AggResult> = all_results.iter().map(|r| aggregate(r)).collect();

    // ---- output ------------------------------------------------------------
    if args.json {
        print_json(binaries, &aggregates, num_items, runs);
    } else {
        let baseline_agg = &aggregates[0];
        for (i, (binary, agg)) in binaries.iter().zip(&aggregates).enumerate() {
            print_human(
                binary,
                agg,
                num_items,
                if binaries.len() > 1 { Some(baseline_agg) } else { None },
                i == 0,
            );
        }

        // Summary comparison table when multiple binaries
        if binaries.len() > 1 {
            println!("\n=== Comparison Summary (vs baseline: {}) ===", binaries[0]);
            let header = format!(
                "{:<40} {:>12} {:>10} {:>14} {:>10} {:>14} {:>10} {:>12} {:>10}",
                "Binary", "Avg time", "Δ time", "Avg rate", "Δ rate", "Avg mem (MB)", "Δ mem", "Avg CPU (%)", "Δ CPU",
            );
            println!("{}", header);
            println!("{}", "-".repeat(header.len()));

            for (i, (binary, agg)) in binaries.iter().zip(&aggregates).enumerate() {
                let t = fmt_opt(agg.avg_time, |v| format!("{:.3}s", v));
                let r = fmt_opt(agg.avg_rate, |v| format!("{:.0}", v));
                let m = fmt_opt(agg.avg_mem, |v| format!("{:.1}", v / 1024.0));
                let c = fmt_opt(agg.avg_cpu, |v| format!("{:.1}", v));

                let (dt, dr, dm, dc) = if i == 0 {
                    (
                        "baseline".into(),
                        "baseline".into(),
                        "baseline".into(),
                        "baseline".into(),
                    )
                } else {
                    (
                        pct(aggregates[0].avg_time, agg.avg_time),
                        pct(aggregates[0].avg_rate, agg.avg_rate),
                        pct(aggregates[0].avg_mem, agg.avg_mem),
                        pct(aggregates[0].avg_cpu, agg.avg_cpu),
                    )
                };

                let name = if binary.len() > 40 {
                    Path::new(binary)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(binary)
                        .to_owned()
                } else {
                    binary.clone()
                };

                println!(
                    "{:<40} {:>12} {:>10} {:>14} {:>10} {:>14} {:>10} {:>12} {:>10}",
                    name, t, dt, r, dr, m, dm, c, dc,
                );
            }
        }
    }

    // ---- perf summary ------------------------------------------------------
    if record_perf {
        eprintln!("\n=== Perf recording output ===");
        for (binary, path) in binaries.iter().zip(&perf_files) {
            if let Some(p) = path {
                if Path::new(p).is_file() {
                    eprintln!("  [{}] perf data: {}", binary, p);
                } else {
                    eprintln!("  [{}] perf data not found (perf may have failed)", binary);
                }
            }
        }
    }
}

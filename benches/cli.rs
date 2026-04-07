//! Interactive benchmark binary for skim.
//!
//! Measures ingestion + matching rate by running sk (or any compatible binary)
//! inside a tmux session, streaming generated (or pre-existing) test data into
//! it, and polling the status line until the matched count stabilises.
//!
//! Binary names are resolved to absolute paths via `which` before use, so bare
//! names like `sk` or `fzf` work as long as they are on `$PATH`.
//! ```

use clap::Parser;
use rand::RngExt as _;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Result, Write};
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
/// Prompt string injected via `--prompt` so we can detect first render in the tmux buffer.
const BENCH_PROMPT: &str = "BENCH> ";
/// Timeout for pre-measurement phases (waiting for shell prompt / command echo).
const PRE_MEASUREMENT_TIMEOUT_S: f64 = 15.0;

/// Seconds the matched count must be unchanged before declaring completion.
const REQUIRED_STABLE_S: f64 = 5.0;
/// Hard timeout per run.
const MAX_WAIT_S: f64 = 120.0;
/// Polling interval in milliseconds.
const CHECK_INTERVAL_MS: u64 = 1;

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
    #[command(subcommand)]
    command: Subcommand,
}

#[derive(clap::Subcommand, Debug)]
enum Subcommand {
    /// Generate test data to a file and exit
    #[command(alias = "gen", alias = "g")]
    Generate(GenerateArgs),

    /// Run the benchmark
    #[command(alias = "r")]
    Run(RunArgs),

    /// Plot results from a JSON file produced by one (or multiple concatenated) `run --json` calls
    Plot(PlotArgs),
}

#[derive(clap::Args, Debug)]
struct PlotArgs {
    /// JSON file produced by `run --json` (entries may be concatenated across runs)
    #[arg(short = 'i', long, default_value = "/tmp/bench.json", value_name = "FILE")]
    input: String,

    /// Output image path
    #[arg(short = 'o', long, default_value = "bench.png", value_name = "FILE")]
    output: String,

    /// Image width in pixels
    #[arg(long, default_value_t = 1600u32, value_name = "PX")]
    width: u32,

    /// Image height in pixels
    #[arg(long, default_value_t = 1200u32, value_name = "PX")]
    height: u32,
}

#[derive(clap::Args, Debug)]
struct GenerateArgs {
    /// Output file to write generated items to
    #[arg(short = 'f', long, value_name = "FILE", required = true)]
    file: String,

    /// Number of items to generate
    #[arg(short = 'n', long, default_value_t = DEFAULT_NUM_ITEMS, value_name = "NUM")]
    num_items: u64,
}

#[derive(clap::Args, Debug)]
struct RunArgs {
    /// One or more paths to binaries (default: ./target/release/sk).
    /// When multiple are given they run in round-robin and the first is used
    /// as the baseline for +/- comparisons.
    #[arg(value_name = "BINARY_PATH", default_value = "")]
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

    /// Run the final benchmark run under strace and write the trace to FILE.
    /// Optionally specify the output file (default: auto-named
    /// strace-<binary>-<timestamp>.out).
    #[arg(
        short = 't',
        long,
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "FILE"
    )]
    strace: Option<String>,

    /// Seconds the matched count must remain unchanged before a run is declared
    /// complete (default: 5.0).
    #[arg(short = 's', long, default_value_t = REQUIRED_STABLE_S, value_name = "SECS")]
    stable_secs: f64,

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
                        if let (Some(rss), Some(cpu)) = (parts.next(), parts.next())
                            && let (Ok(mem), Ok(cpu)) = (rss.parse::<u64>(), cpu.parse::<f64>())
                        {
                            let mut s = stats_clone.lock().unwrap();
                            s.peak_mem_kb = s.peak_mem_kb.max(mem);
                            s.peak_cpu = s.peak_cpu.max(cpu);
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
    /// Total items fed to the binary, as reported by the status line.
    total_count: u64,
    peak_mem_kb: Option<u64>,
    peak_cpu: Option<f64>,
    completed: bool,
    perf_file: Option<String>,
    strace_file: Option<String>,
    /// Time from launch until both the prompt+query and the `N/M` status counts
    /// are visible — i.e. `max(prompt_appeared, status_appeared)`.
    startup_s: Option<f64>,
}

/// Try for up to 2 s to find the sk child PID under `pane_pid`.
/// Checks immediately, then sleeps 5 ms between retries (sleep-after pattern
/// so the first successful check adds no artificial delay).
fn find_sk_pid(pane_pid: u32, binary_path: &str) -> u32 {
    for _ in 0..400 {
        if let Ok(o) = Command::new("pgrep")
            .args(["-P", &pane_pid.to_string(), "-f", binary_path])
            .output()
        {
            let text = String::from_utf8_lossy(&o.stdout);
            if let Some(first) = text.trim().lines().next()
                && let Ok(pid) = first.trim().parse::<u32>()
            {
                return pid;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    0
}

/// Return true if the process with the given PID is still alive.
fn process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

// ---------------------------------------------------------------------------
// Dedicated tmux server
// ---------------------------------------------------------------------------

/// A handle to a private tmux server identified by a unique socket name.
///
/// The server is started with a minimal, clean environment so that nothing
/// from the caller (SKIM_DEFAULT_OPTIONS, FZF_DEFAULT_OPTS, HISTFILE, …)
/// can reach the benchmark panes.  The server is killed automatically when
/// this value is dropped.
struct TmuxServer {
    socket: String,
    capture_buf: String,
}

impl TmuxServer {
    fn start() -> Self {
        let socket = format!("skim_bench_{}", std::process::id());
        let capture_buf = NamedTempFile::new()
            .expect("failed to capture temp file")
            .path()
            .to_string_lossy()
            .into_owned();
        let _ = Command::new("tmux")
            .args(["-L", &socket, "start-server"])
            .env_clear()
            .envs(env_vars())
            .output();
        Self { socket, capture_buf }
    }

    fn capture(&self, session_name: &str) -> Result<String> {
        let buf_name = format!("status-{}", session_name);
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "capture-pane", "-b", &buf_name, "-t", session_name])
            .output();
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "save-buffer", "-b", &buf_name, &self.capture_buf])
            .output();
        fs::read_to_string(&self.capture_buf)
    }
    fn new_session(&self, name: &str) -> Result<()> {
        Command::new("tmux")
            .args(["-L", &self.socket, "new-session", "-s", name, "-d"])
            .env_clear()
            .envs(env_vars())
            .status()
            .and(Ok(()))
    }
    fn send_keys(&self, session_name: &str, keys: &str) -> Result<()> {
        Command::new("tmux")
            .args(["-L", &self.socket, "send-keys", "-t", session_name, keys])
            .status()
            .and(Ok(()))
    }
    fn pane_pid(&self, session_name: &str) -> Result<u32> {
        Command::new("tmux")
            .args([
                "-L",
                &self.socket,
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_pid}",
            ])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(0u32)
            })
    }
    fn kill_session(&self, session_name: &str) -> Result<()> {
        Command::new("tmux")
            .args(["-L", &self.socket, "kill-session", "-t", session_name])
            .status()
            .and(Ok(()))
    }
}

impl Drop for TmuxServer {
    fn drop(&mut self) {
        let _ = Command::new("tmux").args(["-L", &self.socket, "kill-server"]).output();
    }
}

// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_once(
    binary_path: &str,
    query: &str,
    tmp_file: &str,
    extra_args: &[String],
    run_index: u32,
    session_suffix: &str,
    perf_output: Option<&str>,
    strace_output: Option<&str>,
    tmux_server: &TmuxServer,
    stable_secs: f64,
) -> Result<RunResult> {
    let session_name = format!("skim_bench_{}_{}_{}", std::process::id(), session_suffix, run_index);

    // Create a detached session in the dedicated bench server.
    tmux_server.new_session(&session_name)?;
    // Build the command string (will be typed into the shell, not executed yet).
    let extra_str = extra_args.join(" ");
    let perf_prefix = match perf_output {
        Some(path) => format!("perf record -o {} -- ", path),
        None => String::new(),
    };
    let strace_prefix = match strace_output {
        Some(path) => format!("strace -C -ttt -o {} -- ", path),
        None => String::new(),
    };
    let cmd_str = format!(
        "cat {} | {}{}{} --prompt '{}' {}",
        tmp_file, perf_prefix, strace_prefix, binary_path, BENCH_PROMPT, extra_str
    );

    // --- Phase 1: wait for the shell to be ready (any pane content appears) --
    // We don't rely on PS1: whatever prompt the shell shows, any non-blank
    // content means the shell is alive and accepting input.
    {
        let phase_start = Instant::now();
        loop {
            thread::sleep(Duration::from_millis(CHECK_INTERVAL_MS));
            if phase_start.elapsed().as_secs_f64() >= PRE_MEASUREMENT_TIMEOUT_S {
                break;
            }
            if tmux_server.capture(&session_name).is_ok_and(|c| !c.trim().is_empty()) {
                break;
            }
        }
    }

    // Type the command into the shell — no Enter yet.
    tmux_server.send_keys(&session_name, &cmd_str)?;

    // --- Phase 2: wait until the typed command is echoed in the pane ---------
    // `--prompt '` is a short, unique substring of cmd_str that never appears
    // in the binary's TUI output, so it reliably signals the command is ready.
    {
        let cmd_marker = "--prompt '";
        let phase_start = Instant::now();
        loop {
            thread::sleep(Duration::from_millis(CHECK_INTERVAL_MS));
            if phase_start.elapsed().as_secs_f64() >= PRE_MEASUREMENT_TIMEOUT_S {
                break;
            }
            if tmux_server.capture(&session_name).is_ok_and(|c| c.contains(cmd_marker)) {
                break;
            }
        }
    }

    // --- Pre-launch setup (before starting the measurement clock) ------------

    // Get the pane PID now — the session exists and the shell is ready, so
    // this is available without sk running yet.
    let pane_pid: u32 = tmux_server.pane_pid(&session_name)?;

    // Spawn a background thread to find the sk child PID and start the resource
    // monitor.  This must not run on the hot path after Enter because
    // find_sk_pid's pgrep polling would inflate the measured startup time.
    let binary_path_owned = binary_path.to_owned();
    let monitor_cell: Arc<Mutex<Option<ResourceMonitor>>> = Arc::new(Mutex::new(None));
    let sk_pid_cell: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    {
        let monitor_cell = Arc::clone(&monitor_cell);
        let sk_pid_cell = Arc::clone(&sk_pid_cell);
        thread::spawn(move || {
            let pid = find_sk_pid(pane_pid, &binary_path_owned);
            *sk_pid_cell.lock().unwrap() = pid;
            if pid > 0 {
                *monitor_cell.lock().unwrap() = Some(ResourceMonitor::start(pid));
            }
        });
    }

    // Compile the status-line regex once, outside the hot polling loop.
    let re = regex::Regex::new(r"(\d+)/(\d+)").expect("valid regex");
    let prompt_with_query = format!("{}{}", BENCH_PROMPT, query);

    // --- Launch: press Return, queue the query, start the measurement clock --
    tmux_server.send_keys(&session_name, "Enter")?;
    if !query.is_empty() {
        tmux_server.send_keys(&session_name, query)?;
    }
    let start = Instant::now();

    // Poll the tmux pane until both matched and total counts stabilise.
    // We do NOT compare against a pre-counted `num_items`; instead we treat
    // any change in either counter as proof that loading is still in progress.
    let mut completed = false;
    let mut matched_count: u64 = 0;
    let mut total_count: u64 = 0;
    let mut prev_matched: u64 = u64::MAX;
    let mut prev_total: u64 = u64::MAX;
    let mut stable_since: Option<Instant> = None;
    let mut last_change_elapsed: Option<Duration> = None;
    let loop_start = Instant::now();

    // Startup measurements: recorded on first observation, relative to `start`.
    let mut startup_prompt_s: Option<f64> = None;
    let mut startup_status_s: Option<f64> = None;

    loop {
        thread::sleep(Duration::from_millis(CHECK_INTERVAL_MS));

        if loop_start.elapsed().as_secs_f64() >= MAX_WAIT_S {
            break;
        }

        // Check whether sk has exited (non-blocking, via /proc).
        let sk_pid = *sk_pid_cell.lock().unwrap();
        if sk_pid > 0 && !process_alive(sk_pid) {
            break;
        }

        let content = match tmux_server.capture(&session_name) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Startup event: prompt with typed query rendered
        if startup_prompt_s.is_none() && content.contains(&prompt_with_query) {
            startup_prompt_s = Some(start.elapsed().as_secs_f64());
        }

        if let Some(caps) = re.captures(&content) {
            let mc: u64 = caps[1].parse().unwrap_or(0);
            let total: u64 = caps[2].parse().unwrap_or(0);

            // Startup event: status counts visible for the first time
            if startup_status_s.is_none() {
                startup_status_s = Some(start.elapsed().as_secs_f64());
            }

            // Only start the stability clock once we have at least one item.
            if total > 0 {
                total_count = total;
                matched_count = mc;

                if mc != prev_matched || total != prev_total {
                    prev_matched = mc;
                    prev_total = total;
                    stable_since = Some(Instant::now());
                    last_change_elapsed = Some(start.elapsed());
                } else if stable_since.is_some_and(|t| t.elapsed().as_secs_f64() >= stable_secs) {
                    completed = true;
                    break;
                }
            }
        }
    }

    let elapsed_s = last_change_elapsed.unwrap_or_else(|| start.elapsed()).as_secs_f64();

    // Send Escape to exit sk
    tmux_server.send_keys(&session_name, "Escape")?;
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

    // Wait for strace to finish writing before killing the session
    if strace_output.is_some() && pane_pid > 0 {
        let strace_wait = Instant::now();
        loop {
            if strace_wait.elapsed().as_secs_f64() >= 15.0 {
                eprintln!("Warning: strace did not exit within 15 s; trace data may be incomplete.");
                break;
            }
            let still_running = Command::new("pgrep")
                .args(["-P", &pane_pid.to_string(), "-f", "strace"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !still_running {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    let monitor = monitor_cell.lock().unwrap().take();
    let (peak_mem_kb, peak_cpu) = monitor.map(ResourceMonitor::join).unwrap_or((None, None));

    let rate = if elapsed_s > 0.0 && total_count > 0 {
        total_count as f64 / elapsed_s
    } else {
        0.0
    };

    let _ = tmux_server.kill_session(&session_name);

    Ok(RunResult {
        elapsed_s,
        rate,
        matched: matched_count,
        total_count,
        peak_mem_kb,
        peak_cpu,
        completed,
        perf_file: perf_output.map(str::to_owned),
        strace_file: strace_output.map(str::to_owned),
        startup_s: match (startup_prompt_s, startup_status_s) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        },
    })
}

fn env_vars() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| k != "HISTFILE" && !(k.starts_with("FZF") || k.starts_with("SKIM")))
        .chain([("SHELL".into(), "/bin/sh".into())])
        .collect()
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
    avg_total_count: Option<f64>,
    // startup: time until both prompt+query and status counts are visible
    avg_startup_s: Option<f64>,
    min_startup_s: Option<f64>,
    max_startup_s: Option<f64>,
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
    let totals: Vec<f64> = done.iter().map(|r| r.total_count as f64).collect();
    let mems: Vec<f64> = done.iter().filter_map(|r| r.peak_mem_kb.map(|v| v as f64)).collect();
    let cpus: Vec<f64> = done.iter().filter_map(|r| r.peak_cpu).collect();
    // Startup metrics are collected from all runs (not just completed ones)
    let startup: Vec<f64> = results.iter().filter_map(|r| r.startup_s).collect();

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
        avg_total_count: avg(&totals),
        avg_mem: avg(&mems),
        min_mem: mems.iter().copied().reduce(f64::min),
        max_mem: mems.iter().copied().reduce(f64::max),
        avg_cpu: avg(&cpus),
        min_cpu: cpus.iter().copied().reduce(f64::min),
        max_cpu: cpus.iter().copied().reduce(f64::max),
        avg_startup_s: avg(&startup),
        min_startup_s: startup.iter().copied().reduce(f64::min),
        max_startup_s: startup.iter().copied().reduce(f64::max),
    }
}

// ---------------------------------------------------------------------------
// Binary display name
// ---------------------------------------------------------------------------

/// Return a human-readable `"<name> <version>"` label for a binary.
///
/// * If the resolved path lives under `$PWD/target/` the version is `HEAD`.
/// * Otherwise `binary --version` is executed; its first line is parsed:
///   the executable name is stripped from the front (if present), then
///   everything up to the first whitespace is taken as the version token.
fn binary_display_name(binary_path: &str) -> String {
    let exe_name = Path::new(binary_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(binary_path)
        .to_owned();

    // Binaries built locally (in $PWD/target/) are labelled as HEAD.
    let in_target = std::env::current_dir()
        .ok()
        .map(|cwd| Path::new(binary_path).starts_with(cwd.join("target")))
        .unwrap_or(false);

    if in_target {
        return format!("{} HEAD", exe_name);
    }

    // Run `binary --version` and parse the first line.
    let version = Command::new(binary_path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            // Some tools write the version to stderr instead of stdout.
            let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
            let output = if stdout.trim().is_empty() { stderr } else { stdout };
            let first_line = output.lines().next()?.trim().to_owned();
            // Strip the executable name prefix if present, then take the first token.
            let rest = if let Some(stripped) = first_line.strip_prefix(&exe_name) {
                stripped.trim_start().to_owned()
            } else {
                first_line
            };
            Some(rest.split_whitespace().next()?.to_owned())
        })
        .unwrap_or_else(|| "unknown".into());

    format!("{} {}", exe_name, version)
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

fn print_human(binary_label: &str, agg: &AggResult, baseline: Option<&AggResult>, is_baseline: bool) {
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

    // Matched / total
    println!(
        "Average items matched: {}  (min: {}, max: {}) / {}{}",
        fmt_opt(agg.avg_matched, |v| format!("{:.0}", v)),
        fmt_opt(agg.min_matched, |v| format!("{:.0}", v)),
        fmt_opt(agg.max_matched, |v| format!("{:.0}", v)),
        fmt_opt(agg.avg_total_count, |v| format!("{:.0}", v)),
        cmp(agg.avg_matched, baseline.and_then(|b| b.avg_matched)),
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

    // Startup: time until both prompt+query and status counts are visible
    if agg.avg_startup_s.is_some() {
        println!(
            "Startup time (UI ready): {}  (min: {}, max: {}){}",
            fmt_opt(agg.avg_startup_s, |v| format!("{:.3}s", v)),
            fmt_opt(agg.min_startup_s, |v| format!("{:.3}s", v)),
            fmt_opt(agg.max_startup_s, |v| format!("{:.3}s", v)),
            cmp(agg.avg_startup_s, baseline.and_then(|b| b.avg_startup_s)),
        );
    }
}

// ---------------------------------------------------------------------------
// Markdown table output
// ---------------------------------------------------------------------------

fn shorten_binary(binary: &str) -> String {
    if binary.len() > 40 {
        Path::new(binary)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(binary)
            .to_owned()
    } else {
        binary.to_owned()
    }
}

/// Pad `s` to a display width of `width` characters (measured in Unicode scalar
/// values, not bytes — important for multi-byte chars like `Δ` and `—`).
/// Right-aligns when `right_align` is `true`, otherwise left-aligns.
fn pad_cell(s: &str, width: usize, right_align: bool) -> String {
    let n = s.chars().count();
    let extra = width.saturating_sub(n);
    if right_align {
        format!("{}{}", " ".repeat(extra), s)
    } else {
        format!("{}{}", s, " ".repeat(extra))
    }
}

/// Print a GFM-compatible markdown table that is also column-aligned for
/// terminal readability.  All cell values are pre-rendered, per-column widths
/// are computed from the actual content (in chars, not bytes), and every cell
/// is padded to that width before printing.
///
/// When multiple binaries are provided the first is treated as the baseline and
/// delta (Δ) columns are added for every metric.
fn print_markdown_table(display_names: &[String], aggregates: &[AggResult]) {
    let multi = display_names.len() > 1;
    let has_mem = aggregates.iter().any(|a| a.avg_mem.is_some());
    let has_cpu = aggregates.iter().any(|a| a.avg_cpu.is_some());
    let has_startup = aggregates.iter().any(|a| a.avg_startup_s.is_some());

    // ---- column definitions: (header, right_align) -------------------------
    let mut col_defs: Vec<(&str, bool)> =
        vec![("Binary", false), ("Runs", true), ("Matched", true), ("Avg time", true)];
    if multi {
        col_defs.push(("Δ time", true));
    }
    col_defs.push(("Avg rate", true));
    if multi {
        col_defs.push(("Δ rate", true));
    }
    if has_mem {
        col_defs.push(("Avg mem (MB)", true));
        if multi {
            col_defs.push(("Δ mem", true));
        }
    }
    if has_cpu {
        col_defs.push(("Avg CPU (%)", true));
        if multi {
            col_defs.push(("Δ CPU", true));
        }
    }
    if has_startup {
        col_defs.push(("Startup (s)", true));
        if multi {
            col_defs.push(("Δ startup", true));
        }
    }

    // ---- pre-render all data cells -----------------------------------------
    let baseline = &aggregates[0];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for (i, (display_name, agg)) in display_names.iter().zip(aggregates).enumerate() {
        let name = shorten_binary(display_name);
        let name = if i == 0 && multi {
            format!("**{}** *(baseline)*", name)
        } else {
            name
        };

        let mut row: Vec<String> = vec![
            name,
            format!("{}/{}", agg.completed, agg.runs),
            fmt_opt(agg.avg_matched, |v| format!("{:.0}", v)),
            fmt_opt(agg.avg_time, |v| format!("{:.3}s", v)),
        ];

        if multi {
            row.push(if i == 0 {
                "—".into()
            } else {
                pct(baseline.avg_time, agg.avg_time)
            });
        }

        row.push(fmt_opt(agg.avg_rate, |v| format!("{:.0}", v)));

        if multi {
            row.push(if i == 0 {
                "—".into()
            } else {
                pct(baseline.avg_rate, agg.avg_rate)
            });
        }

        if has_mem {
            row.push(fmt_opt(agg.avg_mem, |v| format!("{:.1}", v / 1024.0)));
            if multi {
                row.push(if i == 0 {
                    "—".into()
                } else {
                    pct(baseline.avg_mem, agg.avg_mem)
                });
            }
        }

        if has_cpu {
            row.push(fmt_opt(agg.avg_cpu, |v| format!("{:.1}%", v)));
            if multi {
                row.push(if i == 0 {
                    "—".into()
                } else {
                    pct(baseline.avg_cpu, agg.avg_cpu)
                });
            }
        }

        if has_startup {
            row.push(fmt_opt(agg.avg_startup_s, |v| format!("{:.3}s", v)));
            if multi {
                row.push(if i == 0 {
                    "—".into()
                } else {
                    pct(baseline.avg_startup_s, agg.avg_startup_s)
                });
            }
        }

        rows.push(row);
    }

    // ---- compute per-column display widths (chars, not bytes) --------------
    let mut widths: Vec<usize> = col_defs.iter().map(|(h, _)| h.chars().count()).collect();
    for row in &rows {
        for (j, cell) in row.iter().enumerate() {
            widths[j] = widths[j].max(cell.chars().count());
        }
    }

    // ---- render a padded table row from a slice of cell strings ------------
    let render_row = |cells: &[String]| -> String {
        let padded: Vec<String> = cells
            .iter()
            .zip(&col_defs)
            .zip(&widths)
            .map(|((cell, &(_, right)), &w)| pad_cell(cell, w, right))
            .collect();
        format!("| {} |", padded.join(" | "))
    };

    // ---- header ------------------------------------------------------------
    let headers: Vec<String> = col_defs.iter().map(|(h, _)| h.to_string()).collect();
    println!("{}", render_row(&headers));

    // ---- separator (dashes sized to column width, alignment markers) -------
    let seps: Vec<String> = col_defs
        .iter()
        .zip(&widths)
        .map(|(&(_, right), &w)| {
            // Each separator cell is exactly `w` chars wide so it lines up
            // with the padded header and data cells above and below it.
            if right {
                format!("{}:", "-".repeat(w.saturating_sub(1)))
            } else {
                format!(":{}", "-".repeat(w.saturating_sub(1)))
            }
        })
        .collect();
    println!("| {} |", seps.join(" | "));

    // ---- data rows ---------------------------------------------------------
    for row in &rows {
        println!("{}", render_row(row));
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
    display_name: String,
    runs: u32,
    completed_runs: usize,
    items_matched: JsonMinMaxAvg,
    items_total: Option<f64>,
    time_s: JsonMinMaxAvg,
    items_per_second: JsonMinMaxAvg,
    peak_memory_kb: JsonMinMaxAvg,
    peak_cpu: JsonMinMaxAvg,
    /// Time from launch until both prompt+query and N/M status counts are visible.
    startup_s: JsonMinMaxAvg,
}

fn build_json_entry(binary: &str, display_name: &str, agg: &AggResult, runs: u32) -> JsonEntry {
    JsonEntry {
        binary: binary.to_owned(),
        display_name: display_name.to_owned(),
        runs,
        completed_runs: agg.completed,
        items_matched: JsonMinMaxAvg {
            avg: agg.avg_matched,
            min: agg.min_matched,
            max: agg.max_matched,
        },
        items_total: agg.avg_total_count,
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
        startup_s: JsonMinMaxAvg {
            avg: agg.avg_startup_s,
            min: agg.min_startup_s,
            max: agg.max_startup_s,
        },
    }
}

fn print_json(binaries: &[String], display_names: &[String], aggregates: &[AggResult], runs: u32) {
    let entries: Vec<JsonEntry> = binaries
        .iter()
        .zip(display_names)
        .zip(aggregates)
        .map(|((b, dn), a)| build_json_entry(b, dn, a, runs))
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

fn strace_path_for(binary: &str, explicit: &str) -> String {
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
    format!("strace-{}-{}.out", base, ts)
}

// ---------------------------------------------------------------------------
// Plot subcommand
// ---------------------------------------------------------------------------

/// Deserialised subset of the JSON entries written by `print_json`.
#[derive(Deserialize)]
struct PlotMinMaxAvg {
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Deserialize)]
struct PlotEntry {
    display_name: String,
    items_total: Option<f64>,
    time_s: PlotMinMaxAvg,
    peak_memory_kb: PlotMinMaxAvg,
    peak_cpu: PlotMinMaxAvg,
    startup_s: PlotMinMaxAvg,
}

/// A single x-point in a min/avg/max band.
struct BandPoint {
    x: f64,
    min: f64,
    avg: f64,
    max: f64,
}

/// All band points for one binary / metric combination.
struct BandSeries {
    name: String,
    points: Vec<BandPoint>,
}

/// Parse all `PlotEntry` values from a file containing one or more concatenated
/// JSON objects or arrays (as produced by repeated `run --json >>` calls).
fn load_plot_entries(path: &str) -> std::io::Result<Vec<PlotEntry>> {
    let content = fs::read_to_string(path)?;
    let mut entries: Vec<PlotEntry> = Vec::new();
    for val in serde_json::Deserializer::from_str(&content).into_iter::<serde_json::Value>() {
        match val.ok() {
            Some(serde_json::Value::Array(arr)) => {
                entries.extend(arr.into_iter().filter_map(|v| serde_json::from_value(v).ok()));
            }
            Some(obj @ serde_json::Value::Object(_)) => {
                if let Ok(e) = serde_json::from_value(obj) {
                    entries.push(e);
                }
            }
            _ => {}
        }
    }
    Ok(entries)
}

/// Group entries into per-binary band series for one metric.
/// `extract` returns `(min, avg, max)` or `None` to skip the entry.
fn collect_bands<F>(entries: &[PlotEntry], extract: F) -> Vec<BandSeries>
where
    F: Fn(&PlotEntry) -> Option<(f64, f64, f64)>,
{
    use std::collections::{BTreeMap, HashMap};
    let mut map: HashMap<String, BTreeMap<u64, BandPoint>> = HashMap::new();
    for e in entries {
        let x = match e.items_total {
            Some(v) if v > 0.0 => v,
            _ => continue,
        };
        let (mn, avg, mx) = match extract(e) {
            Some(t) => t,
            None => continue,
        };
        map.entry(e.display_name.clone()).or_default().insert(
            x as u64,
            BandPoint {
                x,
                min: mn,
                avg,
                max: mx,
            },
        );
    }
    let mut series: Vec<BandSeries> = map
        .into_iter()
        .map(|(name, pts)| BandSeries {
            name,
            points: pts.into_values().collect(),
        })
        .collect();
    series.sort_by(|a, b| a.name.cmp(&b.name));
    series
}

fn cmd_plot(args: &PlotArgs) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use gnuplot::{
        AlignType::*, AutoOption::Fix, AxesCommon, BorderLocation2D::*, Caption, Color, Figure, FillAlpha,
        LabelOption::TextColor, LegendOption::Placement, LineWidth,
    };

    // ── Catppuccin Mocha dark palette ─────────────────────────────────────────
    // Colours are referenced as HTML hex strings throughout.
    const BG: &str = "#1e1e2e"; // base
    const SURFACE: &str = "#31324c"; // approximate surface0 (#313244) — used for grid/border
    const TEXT: &str = "#cdd6f4"; // text
    const PALETTE: &[&str] = &[
        "#89b4fa", // blue
        "#f38ba8", // red
        "#a6e3a1", // green
        "#f9e2af", // yellow
        "#cba6f7", // mauve
        "#94e2d5", // teal
        "#fab387", // peach
    ];

    let entries = load_plot_entries(&args.input).map_err(|e| format!("cannot read '{}': {}", args.input, e))?;
    if entries.is_empty() {
        return Err(format!("no valid benchmark entries found in '{}'", args.input).into());
    }

    // ── Build per-metric band series ─────────────────────────────────────────
    let time_bands = collect_bands(&entries, |e| Some((e.time_s.min?, e.time_s.avg?, e.time_s.max?)));
    let cpu_bands = collect_bands(&entries, |e| Some((e.peak_cpu.min?, e.peak_cpu.avg?, e.peak_cpu.max?)));
    let mem_bands = collect_bands(&entries, |e| {
        Some((
            e.peak_memory_kb.min? / 1024.0,
            e.peak_memory_kb.avg? / 1024.0,
            e.peak_memory_kb.max? / 1024.0,
        ))
    });
    let startup_bands = collect_bands(&entries, |e| {
        Some((e.startup_s.min?, e.startup_s.avg?, e.startup_s.max?))
    });

    // ── x range ──────────────────────────────────────────────────────────────
    let all_x: Vec<f64> = entries
        .iter()
        .filter_map(|e| e.items_total)
        .filter(|&v| v > 0.0)
        .collect();
    if all_x.is_empty() {
        return Err("no item-count data".into());
    }
    let x_min = all_x.iter().copied().fold(f64::INFINITY, f64::min);
    let x_max = all_x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let x_lo = x_min * 0.5;
    let x_hi = x_max * 2.0;

    // Helper: compute (min, max) across all points in a set of band series.
    let y_extent = |bands: &[BandSeries]| -> (f64, f64) {
        let mn = bands
            .iter()
            .flat_map(|s| s.points.iter())
            .map(|p| p.min)
            .fold(f64::INFINITY, f64::min);
        let mx = bands
            .iter()
            .flat_map(|s| s.points.iter())
            .map(|p| p.max)
            .fold(f64::NEG_INFINITY, f64::max);
        (mn, mx)
    };

    // ── Helper: draw band series onto an axes object ─────────────────────────
    // Draws a translucent filled region between min and max, plus a solid avg
    // line for each BandSeries.
    let draw_bands = |axes: &mut gnuplot::Axes2D, bands: &[BandSeries]| {
        for (band, &color) in bands.iter().zip(PALETTE.iter().cycle()) {
            if band.points.len() < 2 {
                continue;
            }
            let xs: Vec<f64> = band.points.iter().map(|p| p.x).collect();
            let ys_lo: Vec<f64> = band.points.iter().map(|p| p.min).collect();
            let ys_hi: Vec<f64> = band.points.iter().map(|p| p.max).collect();
            // Filled band (min → max), no legend entry.
            axes.fill_between(
                xs.iter().copied(),
                ys_lo.iter().copied(),
                ys_hi.iter().copied(),
                &[Color(color.into()), FillAlpha(0.22), Caption("")],
            );
            // Average line with legend entry.
            let ys_avg: Vec<f64> = band.points.iter().map(|p| p.avg).collect();
            axes.lines(
                xs.iter().copied(),
                ys_avg.iter().copied(),
                &[Color(color.into()), LineWidth(2.0), Caption(band.name.as_str())],
            );
        }
    };

    // ── Dark-theme gnuplot pre-commands ───────────────────────────────────────
    // These raw gnuplot commands run before any plot-specific commands and apply
    // the Catppuccin Mocha colour scheme globally:
    //   • canvas/plot background
    //   • border, axes and tick colours
    //   • grid line colours
    //   • legend (key) box fill and border
    //   • default text colour
    let pre = format!(
        "set border lc rgb '{surf}'\n\
         set tics textcolor rgb '{text}'\n\
         set xlabel textcolor rgb '{text}'\n\
         set ylabel textcolor rgb '{text}'\n\
         set title  textcolor rgb '{text}'\n\
         set grid lc rgb '{surf}'\n\
         set key opaque fc rgb '{bg}'\n\
         set key box lc rgb '{surf}'\n\
         set key textcolor rgb '{text}'",
        bg = BG,
        surf = SURFACE,
        text = TEXT,
    );

    // ── Build the figure ─────────────────────────────────────────────────────
    let mut fg = Figure::new();
    // The pngcairo terminal lets us set a background color via the `background`
    // keyword; we embed the pixel dimensions here as well.
    fg.set_terminal(
        &format!("pngcairo size {},{} background '{}'", args.width, args.height, BG),
        &args.output,
    );
    fg.set_pre_commands(&pre);
    fg.set_multiplot_layout(2, 2).set_title("Benchmark Results");

    // Shared label options: text in TEXT colour.
    let lbl = &[TextColor(TEXT)];

    // ── Panel 0: Total Time — log x, log y ───────────────────────────────────
    {
        let (mn, mx) = y_extent(&time_bands);
        let y_lo = (mn * 0.5).max(1e-9);
        let y_hi = mx * 2.0;
        let axes = fg.axes2d();
        axes.set_title("Total Time", lbl)
            .set_x_label("Items", lbl)
            .set_y_label("Time (s)", lbl)
            .set_border(true, &[Bottom, Left, Top, Right], &[Color(SURFACE)])
            .set_x_log(Some(10.0))
            .set_y_log(Some(10.0))
            .set_x_range(Fix(x_lo), Fix(x_hi))
            .set_y_range(Fix(y_lo), Fix(y_hi))
            .set_x_grid(true)
            .set_y_grid(true)
            // Legend at upper-left inside the graph, matching the original.
            .set_legend(
                gnuplot::Coordinate::Graph(0.02),
                gnuplot::Coordinate::Graph(0.98),
                &[Placement(AlignLeft, AlignTop)],
                lbl,
            );
        draw_bands(axes, &time_bands);
    }

    // ── Panel 1: Peak CPU — log x, linear y ──────────────────────────────────
    {
        let (_, mx) = y_extent(&cpu_bands);
        let y_hi = (mx * 1.25).max(100.0);
        let axes = fg.axes2d();
        axes.set_title("Peak CPU", lbl)
            .set_x_label("Items", lbl)
            .set_y_label("CPU (%)", lbl)
            .set_border(true, &[Bottom, Left, Top, Right], &[Color(SURFACE)])
            .set_x_log(Some(10.0))
            .set_x_range(Fix(x_lo), Fix(x_hi))
            .set_y_range(Fix(0.0), Fix(y_hi))
            .set_x_grid(true)
            .set_y_grid(true)
            .set_legend(
                gnuplot::Coordinate::Graph(0.02),
                gnuplot::Coordinate::Graph(0.98),
                &[Placement(AlignLeft, AlignTop)],
                lbl,
            );
        draw_bands(axes, &cpu_bands);
    }

    // ── Panel 2: Peak Memory — log x, linear y ───────────────────────────────
    {
        let (_, mx) = y_extent(&mem_bands);
        let y_hi = (mx * 1.25).max(1.0);
        let axes = fg.axes2d();
        axes.set_title("Peak Memory", lbl)
            .set_x_label("Items", lbl)
            .set_y_label("Memory (MB)", lbl)
            .set_border(true, &[Bottom, Left, Top, Right], &[Color(SURFACE)])
            .set_x_log(Some(10.0))
            .set_x_range(Fix(x_lo), Fix(x_hi))
            .set_y_range(Fix(0.0), Fix(y_hi))
            .set_x_grid(true)
            .set_y_grid(true)
            .set_legend(
                gnuplot::Coordinate::Graph(0.02),
                gnuplot::Coordinate::Graph(0.98),
                &[Placement(AlignLeft, AlignTop)],
                lbl,
            );
        draw_bands(axes, &mem_bands);
    }

    // ── Panel 3: Startup Time — log x, linear y ──────────────────────────────
    {
        let (_, mx) = y_extent(&startup_bands);
        let y_hi = (mx * 1.25).max(0.01);
        let axes = fg.axes2d();
        axes.set_title("Startup Time", lbl)
            .set_x_label("Items", lbl)
            .set_y_label("Time (s)", lbl)
            .set_border(true, &[Bottom, Left, Top, Right], &[Color(SURFACE)])
            .set_x_log(Some(10.0))
            .set_x_range(Fix(x_lo), Fix(x_hi))
            .set_y_range(Fix(0.0), Fix(y_hi))
            .set_x_grid(true)
            .set_y_grid(true)
            .set_legend(
                gnuplot::Coordinate::Graph(0.02),
                gnuplot::Coordinate::Graph(0.98),
                &[Placement(AlignLeft, AlignTop)],
                lbl,
            );
        draw_bands(axes, &startup_bands);
    }

    // Send all commands to gnuplot and wait for it to finish writing the PNG.
    // `show_and_keep_running` spawns gnuplot (if not yet spawned), pipes all
    // the plot commands, and returns.  The subsequent `close()` sends "quit"
    // and waits for the process to exit (ensuring the output file is flushed).
    fg.show_and_keep_running().map_err(|e| {
        format!("gnuplot not found or failed to start: {e}\nMake sure gnuplot is installed and available in PATH.")
    })?;
    fg.close();
    eprintln!("Plot written to '{}'", args.output);
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // `cargo bench` injects `--bench` into argv for harness=false benches;
    // strip it before clap sees it so it doesn't land in `extra_args` or
    // cause an "unexpected argument" error.
    let raw: Vec<String> = std::env::args().filter(|a| a != "--bench").collect();
    let args = Args::parse_from(raw);

    // ---- generate / plot subcommands (exit early) --------------------------
    let run_args = match args.command {
        Subcommand::Generate(ref g) => {
            eprintln!("Generating {} items to {} ...", g.num_items, g.file);
            generate_test_data(&g.file, g.num_items).expect("failed to write test data");
            eprintln!("Generated {} items successfully", g.num_items);
            return Ok(());
        }
        Subcommand::Plot(ref p) => {
            if let Err(e) = cmd_plot(p) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return Ok(());
        }
        Subcommand::Run(ref r) => r,
    };

    let mut binaries = run_args.binaries.clone();
    if binaries.is_empty() || (binaries.len() == 1 && binaries[0].is_empty()) {
        binaries = vec![DEFAULT_BINARY.to_owned()];
    }

    // Resolve every binary to an absolute path, the same as replacing
    // `<name>` with `$(which <name>)` at the call site.
    for binary in &mut binaries {
        match which::which(&*binary) {
            Ok(resolved) => *binary = resolved.to_string_lossy().into_owned(),
            Err(e) => {
                eprintln!("Error: cannot resolve binary '{}': {}", binary, e);
                std::process::exit(1);
            }
        }
    }

    // Compute a human-readable display name for each binary once, up front.
    let display_names: Vec<String> = binaries.iter().map(|b| binary_display_name(b)).collect();

    // ---- prepare input data -----------------------------------------------
    let (tmp_file_path, _tmp_file_handle, num_items) = if let Some(ref path) = run_args.file {
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
        generate_test_data(&path, run_args.num_items).expect("failed to generate test data");
        (path, Some(tmp), run_args.num_items)
    };

    let query = &run_args.query;
    let runs = run_args.runs;
    let warmup = run_args.warmup;
    let extra_args = &run_args.extra_args;
    let record_perf = run_args.perf.is_some();
    let perf_explicit = run_args.perf.as_deref().unwrap_or("");
    let record_strace = run_args.strace.is_some();
    let strace_explicit = run_args.strace.as_deref().unwrap_or("");

    // ---- header ------------------------------------------------------------
    eprintln!("=== Skim Ingestion + Matching Benchmark ===");
    eprintln!(
        "Binaries: {} | Items: {} | Query: '{}' | Warmup: {} | Runs: {} (per binary)",
        display_names.join(", "),
        num_items,
        query,
        warmup,
        runs,
    );
    if run_args.file.is_some() {
        eprintln!("Input file: {}", tmp_file_path);
    }
    if !extra_args.is_empty() {
        eprintln!(
            "Extra args: {}",
            extra_args
                .iter()
                .map(|arg| String::from_utf8(shell_quote::Sh::quote_vec(arg)).unwrap())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    if record_perf {
        eprintln!("Perf recording: enabled (final measured run only)");
    }
    if record_strace {
        eprintln!("Strace recording: enabled (final measured run only)");
    }

    // ---- dedicated tmux server ---------------------------------------------
    // Started once with a clean environment; all benchmark panes run inside
    // it so no ambient SKIM_DEFAULT_OPTIONS / FZF_DEFAULT_OPTS / etc. can
    // affect the results.  Killed automatically when `_tmux_server` is dropped
    // at the end of main.
    let tmux_server = TmuxServer::start();

    // ---- warmup (results discarded) ----------------------------------------
    if warmup > 0 {
        eprintln!("\n=== Warmup ({} run(s) per binary) ===", warmup);
        for (bi, binary) in binaries.iter().enumerate() {
            for wu in 1..=warmup {
                eprintln!("  Warmup {}/{} — {} ...", wu, warmup, display_names[bi]);
                let _ = run_once(
                    binary,
                    query,
                    &tmp_file_path,
                    extra_args,
                    wu,
                    &format!("warmup_b{}", bi),
                    None,
                    None,
                    &tmux_server,
                    run_args.stable_secs,
                )?;
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

    // Determine strace output paths (one per binary, recorded only on last run)
    let strace_files: Vec<Option<String>> = if record_strace {
        binaries
            .iter()
            .map(|binary| {
                let explicit = if binaries.len() == 1 { strace_explicit } else { "" };
                Some(strace_path_for(binary, explicit))
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
                    display_names[bi]
                );
            }

            // Attach perf/strace only on the final run for this binary
            let this_perf = if run_num == runs {
                perf_files[bi].as_deref()
            } else {
                None
            };
            let this_strace = if run_num == runs {
                strace_files[bi].as_deref()
            } else {
                None
            };

            let result = run_once(
                binary,
                query,
                &tmp_file_path,
                extra_args,
                run_num,
                &format!("b{}", bi),
                this_perf,
                this_strace,
                &tmux_server,
                run_args.stable_secs,
            )?;

            if runs > 1 || binaries.len() > 1 {
                eprintln!("Status: {}", if result.completed { "COMPLETED" } else { "TIMEOUT" });
                eprintln!("Items matched: {} / {}", result.matched, result.total_count);
                eprintln!("Total time: {:.3}s", result.elapsed_s);
                eprintln!("Items/second: {:.0}", result.rate);
                if let Some(kb) = result.peak_mem_kb {
                    eprintln!("Peak memory usage: {:.1} MB", kb as f64 / 1024.0);
                }
                if let Some(cpu) = result.peak_cpu {
                    eprintln!("Peak CPU usage: {:.1}%", cpu);
                }
                if let Some(s) = result.startup_s {
                    eprintln!("Startup time (UI ready): {:.3}s", s);
                }
                if let Some(ref pf) = result.perf_file {
                    eprintln!("Perf data: {}", pf);
                }
                if let Some(ref sf) = result.strace_file {
                    eprintln!("Strace output: {}", sf);
                }
            }

            all_results[bi].push(result);
        }
    }

    // ---- aggregate ---------------------------------------------------------
    let aggregates: Vec<AggResult> = all_results.iter().map(|r| aggregate(r)).collect();

    // ---- output ------------------------------------------------------------
    if run_args.json {
        print_json(&binaries, &display_names, &aggregates, runs);
    } else {
        let baseline_agg = &aggregates[0];
        for (i, (display_name, agg)) in display_names.iter().zip(&aggregates).enumerate() {
            print_human(
                display_name,
                agg,
                if binaries.len() > 1 { Some(baseline_agg) } else { None },
                i == 0,
            );
        }

        // Summary table — always shown, markdown-formatted
        if binaries.len() > 1 {
            println!("\n## Comparison Summary (vs baseline: `{}`)\n", display_names[0]);
        } else {
            println!("\n## Results Summary\n");
        }
        print_markdown_table(&display_names, &aggregates);
    }

    // ---- perf summary ------------------------------------------------------
    if record_perf {
        eprintln!("\n=== Perf recording output ===");
        for (display_name, path) in display_names.iter().zip(&perf_files) {
            if let Some(p) = path {
                if Path::new(p).is_file() {
                    eprintln!("  [{}] perf data: {}", display_name, p);
                } else {
                    eprintln!("  [{}] perf data not found (perf may have failed)", display_name);
                }
            }
        }
    }

    // ---- strace summary ----------------------------------------------------
    if record_strace {
        eprintln!("\n=== Strace output ===");
        for (display_name, path) in display_names.iter().zip(&strace_files) {
            if let Some(p) = path {
                if Path::new(p).is_file() {
                    eprintln!("  [{}] strace output: {}", display_name, p);
                } else {
                    eprintln!("  [{}] strace output not found (strace may have failed)", display_name);
                }
            }
        }
    }
    Ok(())
}

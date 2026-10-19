//! Drives one candidate through many shows and writes down what it cost.
//!
//! The candidate is started by this process, so cold start is measured rather than
//! guessed, and the daemon is driven over D-Bus rather than through the global
//! shortcut: a key press cannot be issued at a known instant, and the portal would
//! put a permission dialog in the way of an unattended run.
//!
//! Every number here is wall-clock on a live GNOME session, so it carries whatever
//! else the machine is doing. The percentiles are reported rather than an average for
//! that reason: the shape of the tail is what a dashboard is judged on.

mod proc;

use std::io::{BufRead as _, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, bail};
use clap::Parser;
use glimt_shell::{BUS_NAME, INTERFACE, OBJECT_PATH, Sample, monotonic_us};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Candidate binary name, as the workspace spells it.
    #[arg(long, default_value = "spike-gtk4")]
    candidate: String,

    /// Shows to perform.
    #[arg(long, default_value_t = 200)]
    iterations: usize,

    /// Window shape to ask the candidate for.
    #[arg(long, default_value = "window")]
    mode: String,

    /// Where the built binaries are.
    #[arg(long, default_value = "target/release")]
    bin_dir: PathBuf,

    /// Where to write the report.
    #[arg(long, default_value = "bench-results")]
    out: PathBuf,

    /// How long the window stays up before it is hidden again.
    #[arg(long, default_value_t = 250)]
    shown_ms: u64,

    /// How long to wait after hiding before the next show.
    #[arg(long, default_value_t = 150)]
    hidden_ms: u64,

    /// Seconds of doing nothing, to measure what the daemon costs while it waits.
    #[arg(long, default_value_t = 20)]
    idle_seconds: u64,
}

#[derive(Debug, serde::Serialize)]
struct Report {
    candidate: String,
    mode: String,
    geometry: String,
    iterations: usize,
    cold_start_us: i64,
    binary_bytes: u64,
    idle: proc::Memory,
    /// CPU time burned per minute of sitting hidden.
    idle_cpu_us_per_minute: i64,
    latency: Stats,
    samples: Vec<Sample>,
}

#[derive(Debug, Default, serde::Serialize)]
struct Stats {
    count: usize,
    min_us: i64,
    p50_us: i64,
    p95_us: i64,
    p99_us: i64,
    max_us: i64,
    mean_us: i64,
}

impl Stats {
    fn of(samples: &[Sample]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut sorted: Vec<i64> = samples.iter().map(|sample| sample.latency_us).collect();
        sorted.sort_unstable();

        // Nearest-rank, which for a few hundred samples is the only percentile
        // definition that does not invent a value between two measurements.
        let at = |fraction: f64| {
            let rank = (fraction * sorted.len() as f64).ceil() as usize;
            sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
        };

        Self {
            count: sorted.len(),
            min_us: sorted[0],
            p50_us: at(0.50),
            p95_us: at(0.95),
            p99_us: at(0.99),
            max_us: sorted[sorted.len() - 1],
            mean_us: sorted.iter().sum::<i64>() / sorted.len() as i64,
        }
    }
}

/// Kills the candidate when the run ends, including when it ends badly.
struct Candidate(Child);

impl Drop for Candidate {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let binary = cli.bin_dir.join(&cli.candidate);
    if !binary.exists() {
        bail!("{} does not exist; build it first", binary.display());
    }

    let binary_bytes = std::fs::metadata(&binary)?.len();

    let started = monotonic_us();
    let mut child = Command::new(&binary)
        .arg("--no-portal")
        .args(["--mode", &cli.mode])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .with_context(|| format!("starting {}", binary.display()))?;

    let stderr = child
        .stderr
        .take()
        .context("no stderr from the candidate")?;
    let candidate = Candidate(child);
    let pid = candidate.0.id();

    let geometry = drain_log(stderr)
        .recv_timeout(Duration::from_secs(30))
        .context("the candidate never said it was ready")?;
    let cold_start_us = monotonic_us() - started;

    let connection = zbus::blocking::Connection::session().context("session bus")?;
    let proxy = zbus::blocking::Proxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE)
        .context("reaching the candidate")?;

    // A show before the service has claimed its name is a show that never happens.
    wait_for_service(&proxy)?;

    eprintln!("{} ready in {} ms", cli.candidate, cold_start_us / 1000);

    let idle = measure_idle(pid, cli.idle_seconds)?;
    let idle_cpu_us_per_minute = idle.1;

    // Discarded: the first show pays for the surface being created, which happens once
    // in the life of the process and is not what a key press costs.
    show_hide(&proxy, &cli)?;
    let _ = take_samples(&proxy)?;

    eprintln!("running {} shows", cli.iterations);
    for _ in 0..cli.iterations {
        show_hide(&proxy, &cli)?;
    }

    let samples = take_samples(&proxy)?;
    if samples.len() < cli.iterations {
        eprintln!(
            "warning: {} of {} shows went unmeasured",
            cli.iterations - samples.len(),
            cli.iterations
        );
    }

    let report = Report {
        candidate: cli.candidate.clone(),
        mode: cli.mode.clone(),
        geometry,
        iterations: cli.iterations,
        cold_start_us,
        binary_bytes,
        idle: idle.0,
        idle_cpu_us_per_minute,
        latency: Stats::of(&samples),
        samples,
    };

    let _ = proxy.call::<_, _, ()>("Quit", &());
    drop(candidate);

    std::fs::create_dir_all(&cli.out)?;
    let path = cli.out.join(format!("{}-{}.json", cli.candidate, cli.mode));
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;

    print(&report);
    eprintln!("written to {}", path.display());

    Ok(())
}

/// Drain the candidate.s log for as long as it runs, and report the geometry it names.
///
/// The draining has to continue after the ready line: the read end of the pipe closing
/// turns the candidate.s next log write into a broken pipe, which kills it partway
/// through a run. The geometry is carried into the report so that two candidates cannot
/// be compared without it being visible that they drew the same size.
fn drain_log(stderr: std::process::ChildStderr) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            eprintln!("  {line}");

            if let Some(geometry) = line.split("ready geometry=").nth(1) {
                let _ = sender.send(geometry.trim().to_owned());
            }
        }
    });

    receiver
}

fn wait_for_service(proxy: &zbus::blocking::Proxy<'_>) -> anyhow::Result<()> {
    for _ in 0..100 {
        if proxy.call::<_, _, String>("TakeSamples", &()).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    bail!("the service never claimed {BUS_NAME}")
}

fn show_hide(proxy: &zbus::blocking::Proxy<'_>, cli: &Cli) -> anyhow::Result<()> {
    proxy.call::<_, _, ()>("Show", &(""))?;
    std::thread::sleep(Duration::from_millis(cli.shown_ms));
    proxy.call::<_, _, ()>("Hide", &())?;
    std::thread::sleep(Duration::from_millis(cli.hidden_ms));

    Ok(())
}

fn take_samples(proxy: &zbus::blocking::Proxy<'_>) -> anyhow::Result<Vec<Sample>> {
    let json: String = proxy.call("TakeSamples", &())?;
    serde_json::from_str(&json).context("unreadable samples")
}

fn measure_idle(pid: u32, seconds: u64) -> anyhow::Result<(proc::Memory, i64)> {
    eprintln!("sitting idle for {seconds}s");

    let before = proc::cpu_time_us(pid)?;
    std::thread::sleep(Duration::from_secs(seconds));
    let after = proc::cpu_time_us(pid)?;

    let memory = proc::memory(pid)?;
    let per_minute = (after - before) * 60 / seconds.max(1) as i64;

    Ok((memory, per_minute))
}

fn print(report: &Report) {
    let ms = |us: i64| format!("{:.1}", us as f64 / 1000.0);

    println!();
    println!(
        "{} ({}, {})",
        report.candidate, report.mode, report.geometry
    );
    println!("  cold start        {} ms", ms(report.cold_start_us));
    println!("  binary            {} KiB", report.binary_bytes / 1024);
    println!("  idle RSS          {} MiB", report.idle.rss_kb / 1024);
    println!("  idle PSS          {} MiB", report.idle.pss_kb / 1024);
    println!(
        "  idle CPU          {} ms per minute",
        ms(report.idle_cpu_us_per_minute)
    );
    println!("  shows measured    {}", report.latency.count);
    println!("  show latency p50  {} ms", ms(report.latency.p50_us));
    println!("  show latency p95  {} ms", ms(report.latency.p95_us));
    println!("  show latency p99  {} ms", ms(report.latency.p99_us));
    println!(
        "  show latency      min {} ms, max {} ms",
        ms(report.latency.min_us),
        ms(report.latency.max_us)
    );
}

//! What the kernel will say about a running candidate.
//!
//! Resident size is read as both RSS and PSS because the two candidates are not
//! comparable on RSS alone: GTK's libraries are already mapped into the GNOME session,
//! so a large part of what RSS attributes to the process is shared with programs that
//! would be running anyway. PSS divides that by the number of processes sharing it,
//! which is the figure that answers "what does keeping this resident cost".

use std::fs;

use anyhow::Context as _;

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub rss_kb: u64,
    pub pss_kb: u64,
}

pub fn memory(pid: u32) -> anyhow::Result<Memory> {
    let rollup =
        fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).context("reading smaps_rollup")?;

    let field = |name: &str| {
        rollup
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse().ok())
            .unwrap_or_default()
    };

    Ok(Memory {
        rss_kb: field("Rss:"),
        pss_kb: field("Pss:"),
    })
}

/// User plus system time so far, in microseconds.
pub fn cpu_time_us(pid: u32) -> anyhow::Result<i64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).context("reading stat")?;

    // The second field is the executable name in parentheses and may itself contain
    // spaces, so the fields after it are counted from the closing one.
    let tail = stat
        .rsplit_once(')')
        .context("unparsable stat line")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();

    // utime and stime are fields 14 and 15 of the line, which is 11 and 12 of what is
    // left after the name.
    let ticks: i64 = [11, 12]
        .iter()
        .filter_map(|index| tail.get(*index))
        .filter_map(|value| value.parse::<i64>().ok())
        .sum();

    Ok(ticks * 1_000_000 / clock_ticks())
}

fn clock_ticks() -> i64 {
    // SAFETY: sysconf takes a constant and returns a long; it touches nothing.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

    if ticks > 0 { ticks } else { 100 }
}

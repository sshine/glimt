//! What the daemon and its clients have to agree on, and nothing else.
//!
//! Three names and a measurement. This is its own crate because the client that sends
//! `toggle` has no business carrying a Scheme runtime, a TLS stack and a JIT: those
//! arrive through the calculator, which the daemon needs and a client does not. Keeping
//! the agreement separate is what lets `glimtctl` be a small static binary.

/// The name the daemon claims on the session bus.
pub const BUS_NAME: &str = "dk.simonshine.Glimt";
pub const OBJECT_PATH: &str = "/dk/simonshine/Glimt";
pub const INTERFACE: &str = "dk.simonshine.Glimt1";

/// One show, from the request arriving to the compositor saying it is on screen.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub requested_at: i64,
    pub presented_at: i64,
    /// Microseconds between the two, which is the headline number.
    pub latency_us: i64,
}

/// CLOCK_MONOTONIC in microseconds.
///
/// Both frontends report presentation against this clock: GTK's frame timings come from
/// `g_get_monotonic_time`, and `wp_presentation` reports whichever clock it advertises,
/// which is CLOCK_MONOTONIC on every compositor that implements it. Using `Instant`
/// instead would be the same clock with no way to subtract the compositor's timestamp
/// from it.
pub fn monotonic_us() -> i64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    // SAFETY: clock_gettime writes to the timespec it is given and nothing else.
    // CLOCK_MONOTONIC is always available on Linux.
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }

    ts.tv_sec * 1_000_000 + ts.tv_nsec / 1_000
}

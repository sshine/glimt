//! Everything the two spikes share below the drawing.
//!
//! A graphics toolkit owns the main thread and will not give it back, so the daemon is
//! split: the toolkit's own loop stays on the main thread, and D-Bus and the global
//! shortcut run on a worker that reaches the loop through a [`Waker`] the spike
//! supplies. What is left is identical between the two candidates, which is the point.
//! A difference in the measurements then has only one place left to come from.

use std::sync::{Arc, Mutex};

pub mod appearance;
pub mod logging;
pub mod portal;
pub mod service;
pub mod state;
pub mod theme;

pub use service::serve;
pub use state::{Action, AppState, Bar, Response};

/// What the worker asks the toolkit's loop to do.
#[derive(Debug, Clone)]
pub enum Command {
    /// Map the window. `token` is an xdg-activation token, without which mutter's
    /// focus-stealing prevention leaves the window unfocused behind a notification.
    Show {
        token: Option<String>,
        /// CLOCK_MONOTONIC microseconds at which the request arrived, which is the
        /// start of the only interval worth measuring.
        requested_at: i64,
    },
    Hide,
    Toggle {
        token: Option<String>,
        requested_at: i64,
    },
    Quit,
    /// A fresher rate table, from the thread that went to the ECB. Boxed because it is
    /// far larger than every other message and they are all the same size.
    Rates(Box<napkin_math::Rates>),
    /// The desktop switched between its light and dark preference.
    Appearance(appearance::Scheme),
}

pub use glimt_proto::{BUS_NAME, INTERFACE, OBJECT_PATH, Sample, monotonic_us};

/// Collected shows, drained by the benchmark over D-Bus.
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    samples: Arc<Mutex<Vec<Sample>>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called by the spike once the compositor reports the window on screen.
    pub fn record(&self, requested_at: i64, presented_at: i64) {
        let sample = Sample {
            requested_at,
            presented_at,
            latency_us: presented_at - requested_at,
        };

        if let Ok(mut samples) = self.samples.lock() {
            samples.push(sample);
        }
    }

    pub fn drain(&self) -> Vec<Sample> {
        match self.samples.lock() {
            Ok(mut samples) => std::mem::take(&mut *samples),
            Err(_) => Vec::new(),
        }
    }
}

/// How the worker reaches the toolkit's loop.
///
/// GTK has `glib::idle_add_once` and winit has `EventLoopProxy`, and neither can be
/// expressed as the other, so the one thing the spikes cannot share is this.
pub trait Waker: Send + Sync + 'static {
    fn wake(&self, command: Command);
}

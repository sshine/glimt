//! The session-bus service the dashboard is driven by.
//!
//! A D-Bus method is what the benchmark calls, and it is also the fallback trigger for
//! anything that cannot reach the global shortcut portal. The timestamp that starts a
//! measurement is taken here, at the moment the call arrives, so the number includes
//! everything the toolkit does and nothing the caller did.

use std::sync::Arc;

use anyhow::Context as _;

use crate::{Command, Recorder, Waker, monotonic_us};

pub const BUS_NAME: &str = "dk.simonshine.Glimt";
pub const OBJECT_PATH: &str = "/dk/simonshine/Glimt";
pub const INTERFACE: &str = "dk.simonshine.Glimt1";

struct Service {
    waker: Arc<dyn Waker>,
    recorder: Recorder,
}

/// An empty token means the caller had none to give, which is different from a token
/// the compositor went on to reject.
fn token(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

#[zbus::interface(name = "dk.simonshine.Glimt1")]
impl Service {
    fn show(&self, token_str: &str) {
        self.waker.wake(Command::Show {
            token: token(token_str),
            requested_at: monotonic_us(),
        });
    }

    fn hide(&self) {
        self.waker.wake(Command::Hide);
    }

    fn toggle(&self, token_str: &str) {
        self.waker.wake(Command::Toggle {
            token: token(token_str),
            requested_at: monotonic_us(),
        });
    }

    /// Hand the collected shows to the benchmark and forget them, so repeated runs do
    /// not average over each other.
    fn take_samples(&self) -> String {
        serde_json::to_string(&self.recorder.drain()).unwrap_or_else(|_| "[]".to_owned())
    }

    fn quit(&self) {
        self.waker.wake(Command::Quit);
    }
}

/// Claim the name, answer calls, and relay the global shortcut until the process ends.
///
/// Blocks, so the caller runs it on a worker: the toolkit's loop needs the main thread.
pub fn serve(
    waker: Arc<dyn Waker>,
    recorder: Recorder,
    trigger: Option<&str>,
) -> anyhow::Result<()> {
    let connection = zbus::blocking::connection::Builder::session()
        .context("connecting to the session bus")?
        .name(BUS_NAME)
        .context("claiming the bus name")?
        .serve_at(
            OBJECT_PATH,
            Service {
                waker: Arc::clone(&waker),
                recorder,
            },
        )
        .context("publishing the service")?
        .build()
        .context("starting the service")?;

    match trigger {
        // The portal is the only way to be given an activation token the compositor
        // will honour, so a failure here is reported and then lived with: the service
        // still answers calls from glimtctl and from the benchmark.
        Some(trigger) => match crate::portal::listen(&connection, trigger, waker) {
            Ok(()) => Ok(()),
            Err(error) => {
                tracing::warn!("global shortcut unavailable: {error:#}");
                park()
            }
        },
        None => park(),
    }
}

fn park() -> ! {
    loop {
        std::thread::park();
    }
}

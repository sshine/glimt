//! The session-bus service the dashboard is driven by.
//!
//! A D-Bus method is what the benchmark calls, and it is also the fallback trigger for
//! anything that cannot reach the global shortcut portal. The timestamp that starts a
//! measurement is taken here, at the moment the call arrives, so the number includes
//! everything the toolkit does and nothing the caller did.
//!
//! The name is how `glimtctl` finds the daemon, so a failed claim is not treated as a
//! final answer. A user service can start before the session bus it is waiting on, and a
//! connection that is never touched cannot notice that the bus went away underneath it.
//! Both are retried here, for as long as the process lives: the alternative is a daemon
//! that runs, draws and answers nothing.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, bail};

use crate::{Command, Recorder, Waker, monotonic_us};

pub use glimt_proto::{BUS_NAME, INTERFACE, OBJECT_PATH};

/// How long to wait for the bus to answer before deciding it is not ready yet.
///
/// zbus waits forever by default, and a claim that waits forever is indistinguishable
/// from a daemon that is serving: the process is up, the thread is parked inside the
/// build, and no error is ever written. Ten seconds is far longer than a healthy bus
/// needs and short enough that a retry happens while the user is still logging in.
const CLAIM_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait before trying again after a failed claim or a lost connection.
const RETRY: Duration = Duration::from_secs(5);

/// How often to ask the bus whether the name is still ours.
const HEARTBEAT: Duration = Duration::from_secs(10);

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
/// Never returns: a name that cannot be claimed now is retried, because the thing that
/// took it, or the bus that was not up yet, is a temporary condition and the dashboard
/// is useless without the name.
pub fn serve(
    waker: Arc<dyn Waker>,
    recorder: Recorder,
    trigger: Option<&str>,
) -> anyhow::Result<()> {
    let mut failures = 0;

    loop {
        match attach(Arc::clone(&waker), recorder.clone(), trigger) {
            Ok(()) => failures = 0,
            Err(error) => {
                // Once per outage: a second glimt of the user's own, started to see
                // whether the service is alive, should not write a line every retry.
                if failures == 0 {
                    tracing::warn!("not serving {BUS_NAME}: {error:#}; retrying");
                }

                failures += 1;
            }
        }

        std::thread::sleep(RETRY);
    }
}

/// One attempt: claim the name and stay until the claim or the connection is gone.
///
/// `Ok` is the connection ending without anything to report; `Err` is a reason to
/// reconnect, either the attempt failing or the name leaving underneath it.
fn attach(waker: Arc<dyn Waker>, recorder: Recorder, trigger: Option<&str>) -> anyhow::Result<()> {
    let connection = zbus::blocking::connection::Builder::session()
        .context("connecting to the session bus")?
        .method_timeout(CLAIM_TIMEOUT)
        // zbus asks for replacement and allows it by default, which makes the name a
        // prize that the newest process takes from the one that was there first. That
        // is the wrong shape for a daemon: an instance the user starts to look at the
        // dashboard would take the name from the resident one, and on exit leave the
        // name unowned while the resident one stayed up, drawing and answering
        // nothing. First claim wins, and the loser retries until the name is free.
        .allow_name_replacements(false)
        .replace_existing_names(false)
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

    tracing::info!("serving {BUS_NAME}");

    match trigger {
        // The portal is the only way to be given an activation token the compositor
        // will honour, so a failure here is reported and then lived with: the service
        // still answers calls from glimtctl and from the benchmark.
        Some(trigger) => match crate::portal::listen(&connection, trigger, waker) {
            Ok(()) => Ok(()),
            Err(error) => {
                tracing::warn!("global shortcut unavailable: {error:#}");
                hold(&connection)
            }
        },
        None => hold(&connection),
    }
}

/// Keep the connection alive for as long as it owns the name.
///
/// A blocking connection nobody touches is a connection that cannot notice it has been
/// disconnected, so the name is asked for on a heartbeat: `GetNameOwner` fails when the
/// connection is gone and names the wrong connection when the name was taken or lost.
fn hold(connection: &zbus::blocking::Connection) -> anyhow::Result<()> {
    let bus =
        zbus::blocking::fdo::DBusProxy::new(connection).context("asking the bus who owns what")?;
    let ours = connection
        .unique_name()
        .context("the connection has no unique name")?
        .clone();
    let name = zbus::names::BusName::try_from(BUS_NAME).context("naming the service")?;

    loop {
        std::thread::sleep(HEARTBEAT);

        let owner = bus
            .get_name_owner(name.clone())
            .context("asking who owns the name")?;

        if owner != ours {
            bail!("{BUS_NAME} is now {owner}, not {ours}");
        }
    }
}

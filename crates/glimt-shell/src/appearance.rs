//! The session's light or dark preference, as the desktop reports it.
//!
//! Neither candidate owns a theme switch. The desktop has one already, and the point of
//! following it is that the dashboard looks like the rest of the session when it appears
//! rather than like the one application that did not get the message.
//!
//! The XDG settings portal is the source: `org.freedesktop.appearance` carries
//! `color-scheme` on every desktop that implements the setting, and GNOME additionally
//! exposes the GTK theme name there. The theme name is the fallback because a session
//! can be visibly dark without setting `color-scheme` at all: a user who picked a dark
//! GTK theme by hand has `color-scheme = default`, and asking only the first key would
//! call that session light.
//!
//! No portal, or no answer, means light. That is the same default the freedesktop
//! specification gives the key.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use zbus::zvariant::{OwnedValue, Value};

use crate::{Command, Waker};

const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME_INTERFACE: &str = "org.gnome.desktop.interface";
/// How long to wait before reconnecting after the bus or the portal went away.
const RETRY: Duration = Duration::from_secs(10);
/// Long enough for a portal that is still coming up at login, short enough that a
/// missing one does not hold the first window back.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// What the desktop says to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    #[default]
    Light,
    Dark,
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Settings {
    /// The portal's generic reader: a namespace and a key, answered with a variant.
    fn read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    #[zbus(signal)]
    fn setting_changed(
        &self,
        namespace: String,
        key: String,
        value: OwnedValue,
    ) -> zbus::Result<()>;
}

/// Ask the desktop once, blocking for the round trip.
///
/// Called before the first window is built and, on change, from the watcher. A missing
/// portal is not an error worth reporting: the dashboard still has a palette.
pub fn current() -> Scheme {
    connect()
        .ok()
        .and_then(|connection| SettingsProxyBlocking::new(&connection).ok())
        .and_then(|settings| read_scheme(&settings))
        .unwrap_or_default()
}

/// The connection both the one-off read and the watcher use.
fn connect() -> anyhow::Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()
        .context("connecting to the session bus")?
        .method_timeout(READ_TIMEOUT)
        .build()
        .context("building the session bus connection")
}

/// Follow the desktop's preference for the life of the process.
///
/// The watcher runs on its own thread and reaches the toolkit loop through the same
/// [`Waker`] as the D-Bus service, so neither frontend needs a second channel. A portal
/// that is not up yet, or that restarts, is retried; a change that arrives before the
/// first read only costs a repeated, identical message, which the frontend ignores.
pub fn watch(waker: Arc<dyn Waker>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("glimt-appearance".to_owned())
        .spawn(move || run(waker))
        .map(|_| ())
}

fn run(waker: Arc<dyn Waker>) {
    let mut last = None;

    loop {
        if let Err(error) = attach(&waker, &mut last) {
            // Once per outage would need state that the reconnect loop already has;
            // debug is the level at which a session without a portal stays quiet.
            tracing::debug!("not watching the appearance setting: {error:#}");
        }

        std::thread::sleep(RETRY);
    }
}

/// One connection's worth of watching. Returns when the signal stream ends.
fn attach(waker: &Arc<dyn Waker>, last: &mut Option<Scheme>) -> anyhow::Result<()> {
    let connection = connect()?;
    let settings =
        SettingsProxyBlocking::new(&connection).context("the settings portal is not available")?;

    // Subscribed before the first read, so a change between the two is not lost.
    let mut changes = settings
        .receive_setting_changed()
        .context("subscribing to SettingChanged")?;

    announce(waker, last, read_scheme(&settings).unwrap_or_default());

    for signal in changes.by_ref() {
        let args = match signal.args() {
            Ok(args) => args,
            Err(error) => {
                tracing::debug!("undecodable setting change: {error}");
                continue;
            }
        };

        let appearance = args.namespace == APPEARANCE && args.key == "color-scheme";
        let theme = args.namespace == GNOME_INTERFACE && args.key == "gtk-theme";

        if appearance || theme {
            announce(waker, last, read_scheme(&settings).unwrap_or_default());
        }
    }

    Ok(())
}

fn announce(waker: &Arc<dyn Waker>, last: &mut Option<Scheme>, scheme: Scheme) {
    if *last == Some(scheme) {
        return;
    }

    *last = Some(scheme);
    waker.wake(Command::Appearance(scheme));
}

/// Ask for both keys, in the order that keeps an explicit preference ahead of a guess.
fn read_scheme(settings: &SettingsProxyBlocking<'_>) -> Option<Scheme> {
    if let Some(value) = settings
        .read(APPEARANCE, "color-scheme")
        .ok()
        .and_then(|value| u32::try_from(unwrap(value)).ok())
    {
        match value {
            // 1 is prefer-dark and 2 is prefer-light; 0 is no preference.
            1 => return Some(Scheme::Dark),
            2 => return Some(Scheme::Light),
            _ => {}
        }
    }

    let theme = settings
        .read(GNOME_INTERFACE, "gtk-theme")
        .ok()
        .and_then(|value| String::try_from(unwrap(value)).ok())?;

    Some(if theme.to_ascii_lowercase().contains("dark") {
        Scheme::Dark
    } else {
        Scheme::Light
    })
}

/// The portal wraps some settings in a variant and returns others bare, depending on
/// the key. Unwrapping here means the rest of the module only deals with values.
fn unwrap(value: OwnedValue) -> Value<'static> {
    let mut value = Value::from(value);

    while let Value::Value(inner) = value {
        value = *inner;
    }

    value
}

//! The global shortcut, through xdg-desktop-portal.
//!
//! GNOME offers applications no way to grab a key, and a command bound to a GNOME
//! custom shortcut is spawned without input focus, so a window it asks for is refused
//! focus and announced as a notification instead. The GlobalShortcuts portal is the way
//! out: it is the compositor that observes the key, and the `Activated` signal carries
//! an activation token minted by the compositor, which is therefore one it will honour.
//!
//! The portal's request pattern is call-then-await-a-signal on an object whose path the
//! caller has to predict, because the reply can arrive before a subscription made from
//! the returned path would exist.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context as _, bail};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::{Command, Waker, monotonic_us};

#[zbus::proxy(
    interface = "org.freedesktop.portal.GlobalShortcuts",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait GlobalShortcuts {
    fn create_session(
        &self,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn bind_shortcuts(
        &self,
        session: &ObjectPath<'_>,
        shortcuts: Vec<(String, HashMap<String, Value<'_>>)>,
        parent_window: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(signal)]
    fn activated(
        &self,
        session: ObjectPath<'_>,
        shortcut_id: String,
        timestamp: u64,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Request",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Request {
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

const SHORTCUT_ID: &str = "toggle";

/// Register the shortcut and relay every press until the connection ends.
pub fn listen(
    connection: &zbus::blocking::Connection,
    trigger: &str,
    waker: Arc<dyn Waker>,
) -> anyhow::Result<()> {
    let shortcuts = GlobalShortcutsProxyBlocking::new(connection)
        .context("the GlobalShortcuts portal is not available")?;

    let session = create_session(connection, &shortcuts)?;
    bind(connection, &shortcuts, &session, trigger)?;

    tracing::info!("global shortcut bound, preferring {trigger}");

    for signal in shortcuts.receive_activated()? {
        // Taken before anything else so the measurement starts as close to the key
        // press as this process can see it.
        let requested_at = monotonic_us();

        let args = match signal.args() {
            Ok(args) => args,
            Err(error) => {
                tracing::warn!("undecodable activation: {error}");
                continue;
            }
        };

        if args.shortcut_id != SHORTCUT_ID {
            continue;
        }

        waker.wake(Command::Toggle {
            token: args
                .options
                .get("activation_token")
                .and_then(|value| String::try_from(value.clone()).ok()),
            requested_at,
        });
    }

    Ok(())
}

fn create_session(
    connection: &zbus::blocking::Connection,
    shortcuts: &GlobalShortcutsProxyBlocking<'_>,
) -> anyhow::Result<zbus::zvariant::OwnedObjectPath> {
    let handle = "glimt_create";
    let request = request_proxy(connection, handle)?;
    let mut responses = request.receive_response()?;

    shortcuts.create_session(HashMap::from([
        ("handle_token", Value::from(handle)),
        ("session_handle_token", Value::from("glimt_session")),
    ]))?;

    let results = await_response(&mut responses, "CreateSession")?;

    let handle = results
        .get("session_handle")
        .context("CreateSession returned no session_handle")?;
    let handle = String::try_from(handle.clone()).context("session_handle is not a string")?;

    ObjectPath::try_from(handle)
        .context("session_handle is not an object path")
        .map(Into::into)
}

fn bind(
    connection: &zbus::blocking::Connection,
    shortcuts: &GlobalShortcutsProxyBlocking<'_>,
    session: &zbus::zvariant::OwnedObjectPath,
    trigger: &str,
) -> anyhow::Result<()> {
    let handle = "glimt_bind";
    let request = request_proxy(connection, handle)?;
    let mut responses = request.receive_response()?;

    // The trigger is a preference, not a demand: the portal's own settings win, and on
    // GNOME the user is shown a dialog the first time and can choose something else.
    let shortcut = HashMap::from([
        (
            "description".to_owned(),
            Value::from("Show the glimt dashboard"),
        ),
        ("preferred_trigger".to_owned(), Value::from(trigger)),
    ]);

    shortcuts.bind_shortcuts(
        session,
        vec![(SHORTCUT_ID.to_owned(), shortcut)],
        "",
        HashMap::from([("handle_token", Value::from(handle))]),
    )?;

    await_response(&mut responses, "BindShortcuts")?;

    Ok(())
}

/// The object a portal request answers on.
///
/// Predicted rather than taken from the call's return value: the portal may answer
/// before the method returns, and a subscription made afterwards would miss it.
fn request_proxy<'a>(
    connection: &'a zbus::blocking::Connection,
    token: &str,
) -> anyhow::Result<RequestProxyBlocking<'a>> {
    let unique = connection
        .unique_name()
        .context("the connection has no unique name")?;
    let sender = unique.trim_start_matches(':').replace('.', "_");

    RequestProxyBlocking::builder(connection)
        .path(format!(
            "/org/freedesktop/portal/desktop/request/{sender}/{token}"
        ))?
        .build()
        .context("subscribing to the portal request")
}

fn await_response(
    responses: &mut ResponseIterator,
    what: &str,
) -> anyhow::Result<HashMap<String, OwnedValue>> {
    let signal = responses
        .next()
        .with_context(|| format!("{what} never answered"))?;
    let args = signal.args().with_context(|| format!("{what} reply"))?;

    // 0 is success, 1 is the user cancelling, 2 is everything else.
    if args.response != 0 {
        bail!("{what} was refused with code {}", args.response);
    }

    Ok(args.results)
}

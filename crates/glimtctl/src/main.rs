//! Drives a running daemon over the session bus.
//!
//! The global shortcut is the way the dashboard is meant to be opened, and this is
//! everything else: the benchmark's trigger, and a way to see whether the daemon is
//! answering at all without involving the compositor's key handling.

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use glimt_shell::{BUS_NAME, INTERFACE, OBJECT_PATH};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Map the dashboard
    Show,
    /// Unmap it
    Hide,
    /// Whichever of the two it is not already
    Toggle,
    /// Print the shows recorded since the last call, as JSON, and forget them
    Samples,
    /// Ask the daemon to exit
    Quit,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let connection =
        zbus::blocking::Connection::session().context("connecting to the session bus")?;
    let proxy = zbus::blocking::Proxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE)
        .context("reaching the daemon")?;

    // GNOME puts a token in the environment of anything it launches itself. A shell
    // started from a terminal has none, and the daemon is told so rather than being
    // handed an empty string it would have to interpret.
    let token = std::env::var("XDG_ACTIVATION_TOKEN").unwrap_or_default();

    match cli.command {
        Command::Show => proxy.call::<_, _, ()>("Show", &(token.as_str(),))?,
        Command::Hide => proxy.call::<_, _, ()>("Hide", &())?,
        Command::Toggle => proxy.call::<_, _, ()>("Toggle", &(token.as_str(),))?,
        Command::Quit => proxy.call::<_, _, ()>("Quit", &())?,
        Command::Samples => {
            let samples: String = proxy.call("TakeSamples", &())?;
            println!("{samples}");
        }
    }

    Ok(())
}

//! Candidate A: the dashboard drawn with GTK4.
//!
//! The process stays resident and the window stays built. Showing it is a map and
//! hiding it is an unmap, so nothing between the two costs anything at the moment the
//! key is pressed. What this candidate cannot avoid is that GTK unrealizes a hidden
//! window, which throws away the GSK renderer; whether that is measurable against
//! mutter's own map animation is the question the benchmark answers.

mod style;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context as _;
use clap::{Parser, ValueEnum};
use glimt_shell::state::{Action, AppState, Response};
use glimt_shell::{Command, Recorder, Waker, theme};
use gtk4 as gtk;
use gtk4::prelude::*;
use napkin_math::Napkin;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Months in the overview. Six is the half year the reference prints.
    #[arg(long, default_value_t = 6)]
    months: u32,

    /// Shape the window takes on screen.
    #[arg(long, value_enum, default_value_t = Mode::Window)]
    mode: Mode,

    /// Preferred global shortcut, spelled the way the portal spells one.
    #[arg(long, default_value = "SUPER+space")]
    trigger: String,

    /// Do not register the global shortcut; drive the daemon over D-Bus only.
    #[arg(long)]
    no_portal: bool,

    /// Ask for a layer-shell window. Ignored by compositors without the protocol,
    /// which includes mutter.
    #[arg(long)]
    layer_shell: bool,

    /// Start mapped instead of waiting to be asked.
    #[arg(long)]
    visible: bool,

    /// Do not go to the ECB for rates; use whatever is cached or compiled in.
    #[arg(long)]
    offline: bool,

    /// Start with this already typed into the calculator. For screenshots, and for
    /// seeing a result without reaching for the keyboard.
    #[arg(long, value_name = "TEXT")]
    input: Option<String>,

    /// Render once, write a PNG and exit.
    #[arg(long, value_name = "PATH")]
    screenshot: Option<std::path::PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Maximized, which on this desktop means the work area with the top bar left
    /// alone. The only placement mutter offers that is not a guess.
    Window,
    /// A fullscreen toplevel with a translucent backdrop, which is the only way to
    /// control where the dashboard appears on a compositor without layer-shell.
    Fullscreen,
}

/// Posts into GTK's loop from the worker threads.
///
/// GTK widgets are not `Send`, so a worker cannot be handed the window and an idle
/// callback cannot capture it. A channel is the way across: the sender is `Send`, and
/// the receiving future runs on the main thread where the widgets are.
struct ChannelWaker(async_channel::Sender<Command>);

impl Waker for ChannelWaker {
    fn wake(&self, command: Command) {
        // The daemon outliving its own loop is not a case worth handling: if the
        // receiver is gone the process is on its way out.
        let _ = self.0.send_blocking(command);
    }
}

/// What the frame clock has to be asked about repeatedly.
#[derive(Default)]
struct Timing {
    requested_at: Option<i64>,
    frame: Option<i64>,
    attempts: u32,
    connected: bool,
}

fn main() -> anyhow::Result<()> {
    glimt_shell::logging::init();

    let cli = Cli::parse();

    gtk::init().context("starting GTK")?;

    // Around 60 ms to build the Scheme runtime and compile the prelude. Once, here,
    // rather than anywhere a key press waits on.
    let napkin = match Napkin::new() {
        Ok(napkin) => Some(napkin),
        Err(error) => {
            tracing::error!("the calculator did not start: {error}");
            None
        }
    };

    let mut initial = AppState::new(cli.months, napkin);

    for character in cli.input.iter().flat_map(|text| text.chars()) {
        initial.apply(Action::Type(character));
    }

    let state = Rc::new(RefCell::new(initial));

    let display = gdk4::Display::default().context("no display")?;
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&format!("{}{}", style::sheet(), style::overlay_sheet()));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let ui = Rc::new(RefCell::new(ui::Ui::build(&state.borrow())));

    let window = build_window(&ui.borrow(), cli.mode, cli.layer_shell);
    let app = App {
        window,
        state,
        ui,
        timing: Rc::new(RefCell::new(Timing::default())),
        recorder: Recorder::new(),
        blink: Rc::new(RefCell::new(None)),
    };

    app.install_keys();

    if let Some(path) = cli.screenshot {
        return screenshot(&app.window, &path);
    }

    let (sender, receiver) = async_channel::unbounded();
    let waker: Arc<dyn Waker> = Arc::new(ChannelWaker(sender));

    {
        let waker = Arc::clone(&waker);
        let recorder = app.recorder.clone();
        let trigger = (!cli.no_portal).then(|| cli.trigger.clone());
        std::thread::Builder::new()
            .name("glimt-dbus".to_owned())
            .spawn(move || {
                if let Err(error) = glimt_shell::serve(waker, recorder, trigger.as_deref()) {
                    tracing::error!("service stopped: {error:#}");
                }
            })
            .context("starting the service thread")?;
    }

    if !cli.offline {
        let waker = Arc::clone(&waker);
        std::thread::Builder::new()
            .name("glimt-rates".to_owned())
            .spawn(move || glimt_shell::state::refresh_rates_forever(waker))
            .context("starting the rates thread")?;
    }

    {
        let app = app.clone();
        glib::spawn_future_local(async move {
            while let Ok(command) = receiver.recv().await {
                app.handle(command);
            }
        });
    }

    if cli.visible {
        app.show(None, glimt_shell::monotonic_us());
    }

    tracing::info!(
        "ready geometry={}x{}",
        theme::WINDOW_WIDTH as u32,
        theme::WINDOW_HEIGHT as u32
    );
    glib::MainLoop::new(None, false).run();

    Ok(())
}

fn build_window(ui: &ui::Ui, mode: Mode, layer_shell: bool) -> gtk::Window {
    let window = gtk::Window::new();
    window.set_title(Some("glimt"));
    window.add_css_class("glimt");
    window.set_decorated(false);

    match mode {
        Mode::Window => {
            window.set_default_size(theme::WINDOW_WIDTH as i32, theme::WINDOW_HEIGHT as i32);
            // The grid is taller than most screens at its natural size, so the window
            // takes the work area. Maximized rather than fullscreen leaves the top bar
            // where it is, which is the only part of the desktop worth keeping.
            window.maximize();
        }
        Mode::Fullscreen => {
            window.add_css_class("overlay");
            window.fullscreen();
            ui.root.add_css_class("card");
            ui.root.set_halign(gtk::Align::Center);
            ui.root.set_valign(gtk::Align::Center);
        }
    }

    window.set_child(Some(&ui.root));

    if layer_shell {
        apply_layer_shell(&window);
    }

    window
}

fn apply_layer_shell(window: &gtk::Window) {
    use gtk4_layer_shell::{Layer, LayerShell as _};

    if !gtk4_layer_shell::is_supported() {
        tracing::warn!("the compositor has no layer-shell; staying an ordinary toplevel");
        return;
    }

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);
    tracing::info!("layer-shell window on the overlay layer");
}

/// GTK's key events, translated into the shared vocabulary.
///
/// Everything this file decides about the keyboard is here: which key is which action.
/// What an action does lives in `glimt-shell` and is the same for both candidates.
/// Everything the callbacks need, in one place.
///
/// GTK hands a closure nothing, so each one has to carry what it will need, and the
/// list had grown to six handles passed through every function that touches the window.
/// Cloning this is cloning six reference counts.
#[derive(Clone)]
struct App {
    window: gtk::Window,
    state: Rc<RefCell<AppState>>,
    ui: Rc<RefCell<ui::Ui>>,
    timing: Rc<RefCell<Timing>>,
    recorder: Recorder,
    /// The caret timer, which runs only while the window is up.
    blink: Rc<RefCell<Option<glib::SourceId>>>,
}

impl App {
    /// GTK's key events, translated into the shared vocabulary.
    ///
    /// Everything this file decides about the keyboard is here: which key is which
    /// action. What an action does lives in `glimt-shell` and is the same for both
    /// frontends.
    fn install_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        let app = self.clone();

        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let control = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);

            let action = match key {
                gtk::gdk::Key::Escape => Action::Escape,
                gtk::gdk::Key::q | gtk::gdk::Key::Q if control => Action::Root,
                gtk::gdk::Key::Up if control => Action::Weeks(-1),
                gtk::gdk::Key::Down if control => Action::Weeks(1),
                gtk::gdk::Key::Up => Action::Days(-1),
                gtk::gdk::Key::Down => Action::Days(1),
                gtk::gdk::Key::BackSpace => Action::Backspace,
                other => match other.to_unicode() {
                    // A control sequence is not text, and neither is the C0 character a
                    // bare Return or Tab arrives as.
                    Some(character) if !control && !character.is_control() => {
                        Action::Type(character)
                    }
                    _ => return glib::Propagation::Proceed,
                },
            };

            let response = app.state.borrow_mut().apply(action);

            match response {
                Response::Redraw => app.ui.borrow_mut().update(&app.state.borrow()),
                Response::Hide => app.hide(),
                Response::Ignored => return glib::Propagation::Proceed,
            }

            glib::Propagation::Stop
        });

        self.window.add_controller(keys);
    }

    fn handle(&self, command: Command) {
        match command {
            Command::Show {
                token,
                requested_at,
            } => self.show(token, requested_at),

            Command::Hide => self.hide(),

            Command::Toggle {
                token,
                requested_at,
            } => {
                if self.window.is_visible() {
                    self.hide();
                } else {
                    self.show(token, requested_at);
                }
            }

            Command::Rates(rates) => {
                self.state
                    .borrow_mut()
                    .with_napkin_mut(|napkin| napkin.set_rates(*rates));
                self.ui.borrow_mut().update(&self.state.borrow());
            }

            Command::Quit => {
                self.window.close();
                std::process::exit(0);
            }
        }
    }

    fn show(&self, token: Option<String>, requested_at: i64) {
        // The daemon outlives the day it started on.
        let moved_on = self.state.borrow_mut().refresh_today();
        if moved_on {
            self.ui.borrow_mut().update(&self.state.borrow());
        }

        // GTK forwards this to xdg-activation. Without it mutter refuses the focus and
        // announces the window as a notification instead.
        if let Some(token) = token {
            self.window.set_startup_id(&token);
        }

        self.window.present();
        self.start_blink();
        self.arm(requested_at);
    }

    fn hide(&self) {
        self.stop_blink();
        self.window.set_visible(false);
    }

    /// Start the caret blinking, if it is not already.
    ///
    /// Only while the window is up: a hidden dashboard that wakes twice a second to
    /// blink at nobody would show up in the idle measurement as costing something, and
    /// it would be costing it for nothing.
    fn start_blink(&self) {
        if self.blink.borrow().is_some() {
            return;
        }

        let app = self.clone();
        let id = glib::timeout_add_local(
            std::time::Duration::from_millis(theme::CURSOR_BLINK_MS),
            move || {
                app.state.borrow_mut().blink();
                app.ui.borrow_mut().update_input(&app.state.borrow());
                glib::ControlFlow::Continue
            },
        );

        *self.blink.borrow_mut() = Some(id);
    }

    fn stop_blink(&self) {
        if let Some(id) = self.blink.borrow_mut().take() {
            id.remove();
        }
    }

    /// Ask the frame clock to keep producing frames until it can say when this show
    /// reached the screen.
    fn arm(&self, requested_at: i64) {
        let Some(clock) = self.window.frame_clock() else {
            tracing::warn!("no frame clock; this show goes unmeasured");
            return;
        };

        let connect = {
            let mut timing = self.timing.borrow_mut();
            timing.requested_at = Some(requested_at);
            timing.frame = None;
            timing.attempts = 0;

            let first = !timing.connected;
            timing.connected = true;
            first
        };

        if connect {
            let timing = Rc::clone(&self.timing);
            let recorder = self.recorder.clone();
            clock.connect_after_paint(move |clock| observe(clock, &timing, &recorder));
        }

        clock.begin_updating();
    }
}

fn observe(clock: &gdk4::FrameClock, timing: &Rc<RefCell<Timing>>, recorder: &Recorder) {
    let mut state = timing.borrow_mut();

    let Some(requested_at) = state.requested_at else {
        return;
    };

    // The first frame after the request is the one carrying the show. Its presentation
    // time only exists once the compositor has reported back, which is why this returns
    // and waits rather than reading it here.
    let Some(frame) = state.frame else {
        state.frame = Some(clock.frame_counter());
        return;
    };

    state.attempts += 1;

    let presented = clock
        .timings(frame)
        .map(|timings| timings.presentation_time())
        .unwrap_or_default();

    if presented != 0 {
        recorder.record(requested_at, presented);
    } else if state.attempts <= 120 {
        return;
    } else {
        tracing::warn!("no presentation timestamp after {} frames", state.attempts);
    }

    state.requested_at = None;
    state.frame = None;
    clock.end_updating();
}

/// Map the window, let it draw once, write it out and leave.
///
/// The loop has to run first: before the window has been through a frame its widgets
/// have no allocation, and a paintable over them renders nothing at all. Drawn at scale
/// 2 so the two candidates' images can be put side by side.
fn screenshot(window: &gtk::Window, path: &std::path::Path) -> anyhow::Result<()> {
    window.present();

    let clock = window
        .frame_clock()
        .context("the window has no frame clock")?;
    let main_loop = glib::MainLoop::new(None, false);
    let outcome: Rc<RefCell<Option<anyhow::Result<()>>>> = Rc::new(RefCell::new(None));

    {
        let window = window.clone();
        let path = path.to_owned();
        let main_loop = main_loop.clone();
        let outcome = Rc::clone(&outcome);
        let taken = std::cell::Cell::new(false);

        clock.connect_after_paint(move |_| {
            if taken.replace(true) {
                return;
            }

            *outcome.borrow_mut() = Some(capture(&window, &path));
            main_loop.quit();
        });
    }

    main_loop.run();

    outcome
        .take()
        .unwrap_or_else(|| Err(anyhow::anyhow!("the window never drew")))
}

fn capture(window: &gtk::Window, path: &std::path::Path) -> anyhow::Result<()> {
    use gtk::prelude::{PaintableExt as _, SnapshotExt as _};

    // The window rather than its child: the root box's margins live outside its own
    // allocation, so a paintable over the child crops them away.
    let paintable = gtk::WidgetPaintable::new(Some(window));

    let width = f64::from(window.width().max(theme::WINDOW_WIDTH as i32));
    let height = f64::from(window.height().max(theme::WINDOW_HEIGHT as i32));

    let snapshot = gtk::Snapshot::new();
    snapshot.scale(2.0, 2.0);
    paintable.snapshot(&snapshot, width, height);

    let node = snapshot.to_node().context("nothing was drawn")?;
    let renderer = window
        .native()
        .and_then(|native| native.renderer())
        .context("the window has no renderer")?;

    let texture = renderer.render_texture(&node, None);
    texture
        .save_to_png(path)
        .map_err(|error| anyhow::anyhow!("writing the screenshot: {error}"))?;

    println!("{}", path.display());

    Ok(())
}

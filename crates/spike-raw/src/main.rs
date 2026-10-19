//! Candidate B: the dashboard painted into shared memory, with no toolkit under it.
//!
//! Where the GTK candidate hides a window, this one detaches the buffer, which is what
//! unmaps a Wayland surface. Everything else stays: the shared-memory pool, the painted
//! pixels, the font system and the shaped glyph cache. A show is then a commit, a
//! configure and an attach of a buffer that is already correct.
//!
//! The toolkit was dropped rather than reduced because the two things the comparison
//! needs are both below a toolkit's floor. `wp_presentation` is the only source of the
//! timestamp the benchmark is built on, and `wlr-layer-shell` is the only way this
//! candidate could ever be portable to a compositor that has it. Neither is reachable
//! through winit.

mod draw;
mod text;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use clap::{Parser, ValueEnum};
use glimt_shell::state::{Action, AppState, Response};
use glimt_shell::{Command, Recorder, Waker, theme};
use napkin_math::Napkin;
use smithay_client_toolkit::{
    activation::{ActivationHandler, ActivationState, RequestData},
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    delegate_registry,
    output::{OutputHandler, OutputState},
    presentation_time::{PresentTime, PresentationTimeHandler, PresentationTimeState},
    reexports::calloop::{EventLoop, channel},
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};
use tiny_skia::Pixmap;
use wayland_client::{
    Connection, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};
use wayland_protocols::wp::presentation_time::client::wp_presentation_feedback;

const APP_ID: &str = "dk.simonshine.glimt";

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

    /// Paint once, write a PNG and exit.
    #[arg(long, value_name = "PATH")]
    screenshot: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Maximized, which on this desktop means the work area with the top bar left
    /// alone. The only placement mutter offers that is not a guess.
    Window,
    /// A layer-shell window on the overlay layer, with keyboard focus taken from the
    /// compositor. Refused where the protocol is absent, which includes mutter.
    LayerShell,
}

/// Posts into the calloop loop from the worker threads.
struct ChannelWaker(channel::Sender<Command>);

impl Waker for ChannelWaker {
    fn wake(&self, command: Command) {
        let _ = self.0.send(command);
    }
}

/// Whichever shell the surface was created through.
///
/// Both carry a `wl_surface` and both are committed the same way, so only being
/// configured differs, which is the whole of what portability costs here.
enum Shell {
    Xdg(Window),
    Layer(LayerSurface),
}

impl Shell {
    fn surface(&self) -> &wl_surface::WlSurface {
        match self {
            Shell::Xdg(window) => window.wl_surface(),
            Shell::Layer(layer) => layer.wl_surface(),
        }
    }

    fn commit(&self) {
        match self {
            Shell::Xdg(window) => window.commit(),
            Shell::Layer(layer) => layer.commit(),
        }
    }
}
/// One shared-memory buffer and how far behind the painted picture it has fallen.
struct Surface {
    buffer: Buffer,
    behind: Vec<draw::Region>,
}

struct App {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    activation: Option<ActivationState>,
    presentation: PresentationTimeState,

    shell: Shell,
    pool: SlotPool,
    buffers: Vec<Surface>,
    /// What is sent to the compositor.
    pixmap: Pixmap,
    /// The month grid with nothing drawn over it, kept so that a keystroke copies
    /// rather than repaints.
    base: Option<Pixmap>,
    /// The six months the base was painted for.
    base_window: (i32, u32),
    width: u32,
    height: u32,
    scale: i32,
    painted_scale: i32,
    /// The calendar as it was last painted, which is what the next one is compared
    /// against to work out the damage.
    previous: Option<glimt_cal::Overview>,
    previous_bar: Option<BarSnapshot>,
    /// Regions in which this buffer has fallen behind the painted picture.
    /// A frame callback has been asked for and has not arrived.
    awaiting_frame: bool,
    /// Nothing can be reused: paint the lot.
    full: bool,

    /// Mapped, or on its way there.
    shown: bool,
    /// Set between a show and the compositor reporting it on screen.
    pending: Option<i64>,

    state: AppState,
    text: text::Text,
    recorder: Recorder,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    control: bool,
    exit: bool,
}

fn main() -> anyhow::Result<()> {
    glimt_shell::logging::init();

    let cli = Cli::parse();

    // Around 60 ms to build the Scheme runtime and compile the prelude. Once, here,
    // rather than anywhere a key press waits on.
    let napkin = match Napkin::new() {
        Ok(napkin) => Some(napkin),
        Err(error) => {
            tracing::error!("the calculator did not start: {error}");
            None
        }
    };

    let mut state = AppState::new(cli.months, napkin);

    for character in cli.input.iter().flat_map(|text| text.chars()) {
        state.apply(Action::Type(character));
    }

    if let Some(path) = cli.screenshot {
        return screenshot(&state, &path);
    }

    let connection = Connection::connect_to_env().context("connecting to the compositor")?;
    let (globals, event_queue) =
        registry_queue_init(&connection).context("reading the registry")?;
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("wl_compositor")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm")?;
    let presentation = PresentationTimeState::bind(&globals, &qh);
    let activation = ActivationState::bind(&globals, &qh).ok();

    if activation.is_none() {
        tracing::warn!("no xdg-activation; the window will not be given focus");
    }

    let surface = compositor.create_surface(&qh);
    let shell = match cli.mode {
        Mode::Window => {
            let xdg = XdgShell::bind(&globals, &qh).context("xdg_wm_base")?;
            let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
            window.set_title("glimt");
            window.set_app_id(APP_ID);
            // The grid is taller than most screens at its natural size, so the window
            // takes the work area. Maximized rather than fullscreen leaves the top bar
            // where it is, which is the only part of the desktop worth keeping.
            window.set_maximized();
            Shell::Xdg(window)
        }
        Mode::LayerShell => {
            let layer_shell =
                LayerShell::bind(&globals, &qh).context("the compositor has no wlr-layer-shell")?;
            let layer =
                layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("glimt"), None);
            // Anchored to every edge with no size of its own, which is how a layer
            // surface says it wants the whole output.
            layer.set_anchor(Anchor::all());
            layer.set_size(0, 0);
            layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            Shell::Layer(layer)
        }
    };

    let width = theme::WINDOW_WIDTH as u32;
    let height = theme::WINDOW_HEIGHT as u32;
    let pool = SlotPool::new((width * height * 4) as usize, &shm).context("shm pool")?;
    let pixmap = Pixmap::new(width, height).context("allocating the canvas")?;

    let recorder = Recorder::new();
    let mut app = App {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        activation,
        presentation,
        shell,
        pool,
        buffers: Vec::new(),
        pixmap,
        base: None,
        base_window: (0, 0),
        width,
        height,
        scale: 1,
        painted_scale: 0,
        previous: None,
        previous_bar: None,
        awaiting_frame: false,
        full: true,
        shown: false,
        pending: None,
        state,
        text: text::Text::new(),
        recorder: recorder.clone(),
        keyboard: None,
        control: false,
        exit: false,
    };

    let mut event_loop: EventLoop<App> = EventLoop::try_new().context("the event loop")?;
    let loop_handle = event_loop.handle();
    WaylandSource::new(connection.clone(), event_queue)
        .insert(loop_handle.clone())
        .map_err(|error| anyhow::anyhow!("attaching the wayland source: {error}"))?;

    let (sender, receiver) = channel::channel();
    loop_handle
        .insert_source(receiver, |event, _, app| {
            if let channel::Event::Msg(command) = event {
                app.handle(command);
            }
        })
        .map_err(|error| anyhow::anyhow!("attaching the command source: {error}"))?;

    let waker: Arc<dyn Waker> = Arc::new(ChannelWaker(sender));

    {
        let waker = Arc::clone(&waker);
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

    if cli.visible {
        app.show(None, glimt_shell::monotonic_us());
    }

    tracing::info!(
        "ready geometry={}x{}",
        theme::WINDOW_WIDTH as u32,
        theme::WINDOW_HEIGHT as u32
    );

    // No timeout: a poll every 50ms would show up in the idle measurement as this
    // candidate costing CPU to sit still, which would be a property of the loop rather
    // than of the stack. Quit arrives as an event like anything else.
    while !app.exit {
        // A compositor going away is how this process ends when it is run inside a
        // nested one, which is not a failure worth a stack trace.
        if let Err(error) = event_loop.dispatch(None, &mut app) {
            tracing::info!("compositor connection ended: {error}");
            break;
        }
    }

    Ok(())
}

impl App {
    fn handle(&mut self, command: Command) {
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
                if self.shown {
                    self.hide();
                } else {
                    self.show(token, requested_at);
                }
            }
            Command::Rates(rates) => {
                self.state
                    .with_napkin_mut(|napkin| napkin.set_rates(*rates));
                self.full = true;
            }
            Command::Quit => self.exit = true,
        }
    }

    fn show(&mut self, token: Option<String>, requested_at: i64) {
        if self.shown {
            return;
        }

        // The daemon outlives the day it started on.
        if self.state.refresh_today() {
            self.full = true;
        }

        self.pending = Some(requested_at);
        self.shown = true;

        // The compositor minted this token for the key press, so it is one it will
        // accept as a reason to move focus.
        if let (Some(activation), Some(token)) = (self.activation.as_ref(), token) {
            activation.activate::<App>(self.shell.surface(), token);
        }

        // A commit with no buffer is what asks for a configure, which is the only
        // point at which a surface may be given one.
        self.shell.commit();
    }

    fn hide(&mut self) {
        if !self.shown {
            return;
        }

        self.shell.surface().attach(None, 0, 0);
        self.shell.surface().commit();
        self.shown = false;
        self.pending = None;
    }

    /// Attach what is painted, and ask to be told when it reaches the screen.
    /// Put what changed on screen, and only what changed.
    ///
    /// Repainting and re-uploading the whole surface for a keystroke is what made
    /// typing slow: at this size the buffer is 23 MiB, and handing all of it back made
    /// the compositor recomposite and re-upload the lot for one character in a text
    /// field.
    ///
    /// Two buffers, because the compositor is still holding the one just attached when
    /// the next key arrives. Each carries the regions in which it has fallen behind the
    /// painted picture, which is the last two frames' worth rather than the last one's,
    /// and that is all that has to be copied into it.
    fn present(&mut self, qh: &QueueHandle<Self>) {
        let changed = self.paint();

        for surface in &mut self.buffers {
            surface.behind.extend_from_slice(&changed);
        }

        let Some(index) = self.free_buffer() else {
            // Every buffer is held. The pixmap is already current and the regions are
            // still recorded, so nothing is lost, but something has to come back for
            // them: a frame callback is the compositor saying it is ready for the next
            // one. Without this the last keystroke sits unshown until another arrives,
            // which reads exactly like the lag this was meant to remove.
            if !self.awaiting_frame {
                let surface = self.shell.surface();
                surface.frame(qh, FrameCallbackData(surface.clone()));
                surface.commit();
                self.awaiting_frame = true;
            }

            tracing::debug!("all buffers held; waiting for a frame callback");
            return;
        };

        let regions = std::mem::take(&mut self.buffers[index].behind);
        if regions.is_empty() {
            return;
        }

        let scale = self.scale.max(1) as f32;
        let surface = self.buffers.swap_remove(index);

        if let Some(canvas) = self.pool.canvas(&surface.buffer) {
            for region in &regions {
                swap_region(&self.pixmap, canvas, *region, scale);
            }
        }

        let wl_surface = self.shell.surface();
        wl_surface.set_buffer_scale(self.scale);

        // Only while a show is being timed. Asking on every frame would be harmless and
        // pointless; asking before the commit it refers to is what ties the answer to
        // this content update rather than to some later one.
        if self.pending.is_some() {
            if let Err(error) = self.presentation.feedback(wl_surface, qh) {
                tracing::warn!("no presentation feedback: {error}");
            }
        }

        for region in &regions {
            let (x, y, width, height) =
                region.to_pixels(scale, self.pixmap.width(), self.pixmap.height());
            wl_surface.damage_buffer(x as i32, y as i32, width as i32, height as i32);
        }

        if let Err(error) = surface.buffer.attach_to(wl_surface) {
            tracing::error!("attaching the buffer: {error}");
        } else {
            wl_surface.commit();
        }

        self.buffers.push(surface);
    }

    /// A buffer the compositor is not reading, making a second one if there is room.
    fn free_buffer(&mut self) -> Option<usize> {
        if let Some(index) = self
            .buffers
            .iter()
            .position(|surface| !surface.buffer.slot().has_active_buffers())
        {
            return Some(index);
        }

        // Two is enough to stay a frame ahead of the compositor. A third would only
        // hold another 23 MiB of the same picture.
        if self.buffers.len() >= 2 {
            return None;
        }

        self.allocate().then(|| self.buffers.len() - 1)
    }

    /// Put a new frame on screen because something changed, rather than because the
    /// compositor asked.
    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if self.shown {
            self.present(qh);
        }
    }

    fn pixels(&self) -> (i32, i32) {
        (
            self.width as i32 * self.scale,
            self.height as i32 * self.scale,
        )
    }

    /// Take another buffer at the pixmap's current size.
    ///
    /// A new buffer holds nobody's pixels, so it is behind everywhere and its first
    /// frame copies the whole picture into it. That happens twice per size, not once
    /// per keystroke.
    fn allocate(&mut self) -> bool {
        let (width, height) = self.pixels();

        match self
            .pool
            .create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)
        {
            Ok((buffer, _)) => {
                self.buffers.push(Surface {
                    buffer,
                    behind: vec![draw::Region::new(
                        0.0,
                        0.0,
                        self.width as f32,
                        self.height as f32,
                    )],
                });
                true
            }
            Err(error) => {
                tracing::error!("no buffer: {error}");
                false
            }
        }
    }

    /// Note a new size, and throw away what was painted for the old one.
    fn resized(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;
        self.painted_scale = 0;
        self.base = None;
        self.buffers.clear();
        self.full = true;
    }

    /// Everything that is on screen now but should not be, and the other way round.
    ///
    /// The calendar is rebuilt wholesale on every change because that costs nothing, so
    /// what has to be worked out is which of its days look different from the ones
    /// already painted. A keystroke usually answers: none of them.
    fn changed(&self, layout: draw::Layout) -> Vec<draw::Region> {
        let overview = self.state.overview();
        let mut regions = Vec::new();

        let Some(previous) = self.previous.as_ref() else {
            return vec![layout.everything()];
        };

        // The months on show changed, so the grid underneath is a different picture.
        if previous.months.len() != overview.months.len()
            || previous
                .months
                .first()
                .map(|column| (column.year, column.month))
                != overview
                    .months
                    .first()
                    .map(|column| (column.year, column.month))
        {
            return vec![layout.everything()];
        }

        if previous.strip != overview.strip {
            regions.push(layout.strip_band());
        }

        if self.previous_bar.as_ref() != Some(&snapshot(&self.state.bar())) {
            regions.push(layout.bar_band());
        }

        for (index, (was, now)) in previous.months.iter().zip(&overview.months).enumerate() {
            for (row, (before, after)) in was.days.iter().zip(&now.days).enumerate() {
                if marks_differ(before, after) {
                    regions.push(layout.row_band(index, row));
                }
            }
        }

        regions
    }

    fn paint(&mut self) -> Vec<draw::Region> {
        let scale = self.scale.max(1) as u32;
        let (pixel_width, pixel_height) = (self.width * scale, self.height * scale);

        if self.pixmap.width() != pixel_width || self.pixmap.height() != pixel_height {
            match Pixmap::new(pixel_width, pixel_height) {
                Some(pixmap) => self.pixmap = pixmap,
                None => {
                    tracing::error!("cannot allocate a {scale}x canvas");
                    return Vec::new();
                }
            }
            self.base = None;
            self.full = true;
        }

        let overview = self.state.overview();
        let layout =
            draw::Layout::new(self.width as f32, self.height as f32, overview.months.len());
        let window = overview
            .months
            .first()
            .map(|column| (column.year, column.month))
            .unwrap_or_default();

        if self.base.is_none() || self.base_window != window {
            let started = glimt_shell::monotonic_us();
            let mut base = match Pixmap::new(pixel_width, pixel_height) {
                Some(base) => base,
                None => return Vec::new(),
            };

            draw::base(&mut base, &mut self.text, overview, layout, scale as f32);
            self.base = Some(base);
            self.base_window = window;
            self.full = true;

            tracing::info!(
                "grid painted {pixel_width}x{pixel_height} in {} us",
                glimt_shell::monotonic_us() - started
            );
        }

        let damage = if self.full {
            vec![layout.everything()]
        } else {
            self.changed(layout)
        };

        if damage.is_empty() {
            return Vec::new();
        }

        let started = glimt_shell::monotonic_us();

        // The marks move, so what they were covering has to come back from the grid
        // underneath before the new ones go on top.
        if let Some(base) = self.base.take() {
            for region in &damage {
                copy_region(&base, &mut self.pixmap, *region, scale as f32);
            }
            self.base = Some(base);
        }

        let bar = self.state.bar();
        draw::overlay(
            &mut self.pixmap,
            &mut self.text,
            overview,
            &bar,
            layout,
            scale as f32,
            &damage,
        );

        self.previous_bar = Some(snapshot(&bar));
        self.previous = Some(overview.clone());
        self.painted_scale = self.scale;
        self.full = false;
        let painted = damage;

        tracing::debug!(
            "painted {} region(s) in {} us",
            painted.len(),
            glimt_shell::monotonic_us() - started
        );

        painted
    }

    /// Wayland's keysyms, translated into the shared vocabulary.
    ///
    /// Everything this file decides about the keyboard is here: which key is which
    /// action. What an action does lives in `glimt-shell` and is the same for both
    /// candidates.
    fn action(&self, event: &KeyEvent) -> Option<Action> {
        let control = self.control;

        match event.keysym {
            Keysym::Escape => Some(Action::Escape),
            Keysym::q | Keysym::Q if control => Some(Action::Root),
            Keysym::Up if control => Some(Action::Weeks(-1)),
            Keysym::Down if control => Some(Action::Weeks(1)),
            Keysym::Up => Some(Action::Days(-1)),
            Keysym::Down => Some(Action::Days(1)),
            Keysym::BackSpace => Some(Action::Backspace),
            _ if control => None,
            _ => event
                .utf8
                .as_ref()
                .and_then(|text| text.chars().next())
                .filter(|character| !character.is_control())
                .map(Action::Type),
        }
    }
}

/// The calculator bar as it was painted, for telling whether it needs painting again.
#[derive(PartialEq)]
struct BarSnapshot {
    input: String,
    answer: Option<glimt_shell::state::Answer>,
    faded: bool,
    problem: Option<String>,
}

fn snapshot(bar: &glimt_shell::Bar<'_>) -> BarSnapshot {
    BarSnapshot {
        input: bar.input.to_owned(),
        answer: bar.answer.cloned(),
        faded: bar.faded,
        problem: bar.problem.map(str::to_owned),
    }
}

/// Whether anything drawn over this day changed. The day itself cannot: the grid
/// underneath is repainted only when the months on show change.
fn marks_differ(before: &glimt_cal::Day, after: &glimt_cal::Day) -> bool {
    before.selected != after.selected
        || before.highlighted != after.highlighted
        || before.in_focus_week != after.in_focus_week
}

/// Restore one region of the painted picture from the grid underneath it.
fn copy_region(source: &Pixmap, target: &mut Pixmap, region: draw::Region, scale: f32) {
    let (left, top, width, height) = region.to_pixels(scale, source.width(), source.height());
    if width == 0 || height == 0 {
        return;
    }

    let stride = source.width() as usize * 4;
    let span = width as usize * 4;
    let source = source.data();
    let target = target.data_mut();

    for row in 0..height as usize {
        let at = (top as usize + row) * stride + left as usize * 4;
        target[at..at + span].copy_from_slice(&source[at..at + span]);
    }
}

/// Hand one region to the shared-memory buffer.
///
/// tiny-skia keeps premultiplied RGBA; wl_shm's ARGB8888 is the same bytes with red and
/// blue the other way round on a little-endian machine. Doing this for the whole surface
/// on every keystroke was most of why typing was slow, so it is done for the regions
/// that changed and no others.
fn swap_region(source: &Pixmap, canvas: &mut [u8], region: draw::Region, scale: f32) {
    let (left, top, width, height) = region.to_pixels(scale, source.width(), source.height());
    if width == 0 || height == 0 {
        return;
    }

    let stride = source.width() as usize * 4;
    let span = width as usize * 4;
    let source = source.data();

    for row in 0..height as usize {
        let at = (top as usize + row) * stride + left as usize * 4;

        for (src, dst) in source[at..at + span]
            .chunks_exact(4)
            .zip(canvas[at..at + span].chunks_exact_mut(4))
        {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
    }
}

/// Paint one frame off screen and write it out, with no compositor involved.
fn screenshot(state: &AppState, path: &std::path::Path) -> anyhow::Result<()> {
    let scale = 2.0;
    let width = theme::WINDOW_WIDTH;
    let height = theme::WINDOW_HEIGHT;

    let mut pixmap = Pixmap::new((width * scale) as u32, (height * scale) as u32)
        .context("allocating the canvas")?;
    let mut text = text::Text::new();
    let overview = state.overview();
    let layout = draw::Layout::new(width, height, overview.months.len());

    draw::base(&mut pixmap, &mut text, overview, layout, scale);
    draw::overlay(
        &mut pixmap,
        &mut text,
        overview,
        &state.bar(),
        layout,
        scale,
        &[layout.everything()],
    );

    pixmap.save_png(path).context("writing the screenshot")?;
    println!("{}", path.display());

    Ok(())
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if new_factor == self.scale {
            return;
        }

        self.scale = new_factor;
        self.base = None;
        self.redraw(qh);
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        // The compositor is ready for another frame, which is what a deferred one was
        // waiting for.
        self.awaiting_frame = false;
        self.redraw(qh);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.hide();
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        // mutter names a size in every configure, including the ones that come from
        // remapping an unchanged window. Treating each of those as a resize would
        // repaint a megapixel on the path a key press waits on.
        if let (Some(width), Some(height)) = configure.new_size {
            self.resized(width.get(), height.get());
        }

        self.present(qh);
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) = configure.new_size;
        if width != 0 && height != 0 {
            self.resized(width, height);
        }

        self.present(qh);
    }
}

impl PresentationTimeHandler for App {
    fn presentation_time_state(&mut self) -> &mut PresentationTimeState {
        &mut self.presentation
    }

    fn presented(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _feedback: &wp_presentation_feedback::WpPresentationFeedback,
        _surface: &wl_surface::WlSurface,
        _outputs: Vec<wl_output::WlOutput>,
        time: PresentTime,
        _refresh: u32,
        _seq: u64,
        _flags: WEnum<wp_presentation_feedback::Kind>,
    ) {
        let Some(requested_at) = self.pending.take() else {
            return;
        };

        let presented_at = time.tv_sec as i64 * 1_000_000 + time.tv_nsec as i64 / 1_000;
        self.recorder.record(requested_at, presented_at);
    }

    fn discarded(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        _: &wl_surface::WlSurface,
    ) {
        // A discarded update never reached the screen, so there is nothing to time. The
        // pending show stays pending and the next presentation answers it.
        tracing::debug!("content update discarded");
    }
}

impl ActivationHandler for App {
    type RequestUdata = ();

    fn new_token(&mut self, token: String, _data: &RequestData<()>) {
        if let Some(activation) = self.activation.as_ref() {
            activation.activate::<App>(self.shell.surface(), token);
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(keyboard) => self.keyboard = Some(keyboard),
                Err(error) => tracing::warn!("no keyboard: {error}"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        tracing::info!("keyboard focus taken");
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // Modifiers held when focus left are not held when it comes back.
        self.control = false;
    }

    fn press_key(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let Some(action) = self.action(&event) else {
            return;
        };
        tracing::debug!("key {:?} -> {:?}", event.keysym, action);

        match self.state.apply(action) {
            Response::Redraw => self.redraw(qh),
            Response::Hide => self.hide(),
            Response::Ignored => {}
        }
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Holding an arrow key walks the calendar, which is the one thing here worth
        // repeating.
        self.press_key(conn, qh, keyboard, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.control = modifiers.ctrl;
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(App);

use crate::{
    capture::Capture,
    config::Config,
    hypr::{self, Monitor},
    ipc, render,
};
use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    delegate_simple,
    output::{OutputHandler, OutputState},
    reexports::{calloop::EventLoop, calloop_wayland_source::WaylandSource},
    registry::{ProvidesRegistryState, RegistryState, SimpleGlobal},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::{self, WpViewport},
    wp_viewporter::WpViewporter,
};

struct Overlay {
    output: wl_output::WlOutput,
    monitor: Monitor,
    layer: LayerSurface,
    viewport: WpViewport,
    ready: bool,
    visible: bool,
    pool: SlotPool,
}
impl Drop for Overlay {
    fn drop(&mut self) {
        self.viewport.destroy();
    }
}
impl Overlay {
    fn hide(&mut self) -> Result<()> {
        if self.visible && self.ready {
            let (buffer, canvas) = self.pool.create_buffer(1, 1, 4, wl_shm::Format::Argb8888)?;
            canvas.fill(0);
            self.viewport.set_destination(1, 1);
            self.layer.set_size(1, 1);
            buffer.attach_to(self.layer.wl_surface())?;
            self.layer.wl_surface().damage_buffer(0, 0, 1, 1);
            self.layer.commit();
            self.visible = false;
        }
        Ok(())
    }
}

pub struct State {
    registry: RegistryState,
    outputs: OutputState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,
    viewporter: SimpleGlobal<WpViewporter, 1>,
    layers: Vec<Overlay>,
    pub capture: Option<Capture>,
    pub dirty: bool,
    config: Config,
    cursor: (f32, f32),
    velocity: (f32, f32),
    last_move: Instant,
    last_click: Instant,
    button: Option<bool>,
    held: bool,
    locate: Option<Instant>,
    magnify: bool,
    exit: bool,
    refresh: bool,
    last_capture: Instant,
    capture_monitor: String,
    last_error: Option<String>,
}

pub fn run() -> Result<()> {
    let config = Config::load()?;
    let (_lock, listener) = ipc::listen()?;
    let conn = Connection::connect_to_env().context("Unable to connect to the Wayland desktop")?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let shm = Shm::bind(&globals, &qh)?;
    let capture = globals
        .bind(&qh, 3..=3, ())
        .ok()
        .map(|manager| Capture::new(manager, &shm))
        .transpose()?;
    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh)?,
        layer_shell: LayerShell::bind(&globals, &qh)?,
        viewporter: SimpleGlobal::bind(&globals, &qh)?,
        shm,
        capture,
        layers: Vec::new(),
        dirty: true,
        config,
        cursor: hypr::cursor()?,
        velocity: (0., 0.),
        last_move: Instant::now(),
        last_click: Instant::now() - Duration::from_secs(60),
        button: None,
        held: false,
        locate: None,
        magnify: false,
        exit: false,
        refresh: true,
        last_capture: Instant::now(),
        capture_monitor: String::new(),
        last_error: None,
    };
    queue.roundtrip(&mut state)?;
    state.sync_outputs(&qh)?;
    let mut tray = state
        .config
        .show_tray
        .then(crate::tray::start)
        .transpose()
        .map_err(|e| eprintln!("Tray unavailable: {e:#}"))
        .ok()
        .flatten();
    let mut event_loop: EventLoop<State> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("Wayland event source: {e}"))?;
    let mut next_tick = Instant::now();
    let mut next_monitors = Instant::now() + Duration::from_secs(2);
    let mut next_tray_retry = Instant::now() + Duration::from_secs(30);
    let mut tray_state = (state.config.enabled, state.magnify);
    eprintln!(
        "Omarchy Cursor ready: {} displays, magnifier capture {}",
        state.layers.len(),
        state.capture.is_some()
    );
    while !state.exit {
        event_loop.dispatch(
            Some(next_tick.saturating_duration_since(Instant::now())),
            &mut state,
        )?;
        if Instant::now() < next_tick {
            continue;
        }
        next_tick = Instant::now() + Duration::from_millis(16);
        // Bound requests per tick so a noisy client cannot starve drawing.
        for _ in 0..32 {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            stream.set_read_timeout(Some(Duration::from_millis(30)))?;
            stream.set_write_timeout(Some(Duration::from_millis(30)))?;
            let mut command = String::new();
            if Read::by_ref(&mut stream)
                .take(1024)
                .read_to_string(&mut command)
                .is_ok()
            {
                let reply = state.command(command.trim());
                let _ = stream.write_all(reply.as_bytes());
            }
        }
        if state.refresh || Instant::now() >= next_monitors {
            match state.sync_outputs(&qh) {
                Ok(()) => state.last_error = None,
                Err(e) => state.last_error = Some(e.to_string()),
            }
            next_monitors = Instant::now() + Duration::from_secs(2);
            state.refresh = false;
        }
        match hypr::cursor() {
            Ok(p) => {
                let delta = (p.0 - state.cursor.0, p.1 - state.cursor.1);
                let previous_velocity = state.velocity;
                state.velocity = (
                    state.velocity.0 * 0.7 + delta.0 * 0.3,
                    state.velocity.1 * 0.7 + delta.1 * 0.3,
                );
                if state.config.perspective
                    && (previous_velocity.0 - state.velocity.0).abs()
                        + (previous_velocity.1 - state.velocity.1).abs()
                        > 0.1
                {
                    state.dirty = true;
                }
                if delta.0.abs() + delta.1.abs() > 0.1 {
                    state.last_move = Instant::now();
                    state.dirty = true;
                }
                state.cursor = p;
            }
            Err(e) => {
                state.last_error = Some(e.to_string());
                state.hide()?;
                conn.flush()?;
                continue;
            }
        }
        let active = state.last_click.elapsed().as_secs_f32() < 0.65
            || state.locate.is_some()
            || state.config.auto_hide
            || state.config.attention
            || state.magnify;
        if state.dirty || active {
            state.draw(&qh)?;
            state.dirty = false;
        }
        // Capture after committing the lens's new position: its source stays transparent.
        state.request_capture(&qh);
        let current = (state.config.enabled, state.magnify);
        if !state.config.show_tray {
            if let Some(handle) = tray.take() {
                handle.shutdown();
            }
        } else if tray.is_none() && Instant::now() >= next_tray_retry {
            tray = crate::tray::start().ok();
            next_tray_retry = Instant::now() + Duration::from_secs(30);
        }
        if current != tray_state {
            if let Some(handle) = &tray {
                handle.update(|t| {
                    t.enabled = current.0;
                    t.magnifying = current.1;
                });
            }
            tray_state = current;
        }
        conn.flush()?;
    }
    let _ = std::fs::remove_file(ipc::dir()?.join("control.sock"));
    Ok(())
}

impl State {
    fn command(&mut self, command: &str) -> String {
        self.dirty = true;
        match command {
            "toggle" => {
                self.config.enabled = !self.config.enabled;
                if !self.config.enabled {
                    self.magnify = false;
                }
                if let Err(e) = self.config.save() {
                    return format!("error: {e}");
                }
            }
            "magnify" | "magnify-on" | "magnify-off" => {
                if self.config.magnifier_enabled && self.capture.is_some() {
                    self.magnify = match command {
                        "magnify-on" => true,
                        "magnify-off" => false,
                        _ => !self.magnify,
                    };
                    if let Some(c) = &mut self.capture {
                        c.cancel();
                    }
                } else if command != "magnify-off" {
                    return "error: Magnifier disabled or screen capture unavailable".into();
                }
            }
            "locate" => self.locate = Some(Instant::now()),
            "quit" => self.exit = true,
            "reload" => match Config::load() {
                Ok(c) => {
                    self.config = c;
                    if !self.config.magnifier_enabled {
                        self.magnify = false;
                    }
                }
                Err(e) => return format!("error: {e}"),
            },
            "status" => {}
            _ if command.starts_with("click ") => {
                let words: Vec<_> = command.split_whitespace().collect();
                if words.len() == 3 {
                    self.held = words[2] == "down";
                    self.last_click = Instant::now();
                    if self.held {
                        self.button = Some(words[1] != "right");
                        self.last_click = Instant::now();
                    }
                    self.last_move = Instant::now();
                }
            }
            _ => return "error: unknown command".into(),
        }
        serde_json::json!({"running":true,"enabled":self.config.enabled,"magnifying":self.magnify,"shape":self.config.shape,"radius":self.config.radius,"zoom":self.config.zoom,"toggle_key":self.config.toggle_key,"magnify_key":self.config.magnify_key,"locate_key":self.config.locate_key,"displays":self.layers.iter().map(|l|l.monitor.name.clone()).collect::<Vec<_>>(),"capture_available":self.capture.is_some(),"captured_frames":self.capture.as_ref().map_or(0,|c|c.frames),"capture_error":self.capture.as_ref().and_then(|c|c.error.as_ref()),"error":self.last_error,"held":self.held}).to_string()
    }
    fn sync_outputs(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let monitors = hypr::monitors()?;
        self.layers.retain(|l| {
            monitors.iter().any(|m| m.name == l.monitor.name)
                && self.outputs.outputs().any(|o| o == l.output)
        });
        for m in monitors {
            if let Some(layer) = self.layers.iter_mut().find(|l| l.monitor.name == m.name) {
                if layer.monitor.x != m.x
                    || layer.monitor.y != m.y
                    || layer.monitor.size() != m.size()
                    || layer.monitor.scale != m.scale
                {
                    self.dirty = true;
                }
                layer.monitor = m;
                continue;
            }
            let Some(output) = self
                .outputs
                .outputs()
                .find(|o| self.outputs.info(o).and_then(|i| i.name).as_deref() == Some(&m.name))
            else {
                continue;
            };
            let surface = self.compositor.create_surface(qh);
            let region = smithay_client_toolkit::compositor::Region::new(&self.compositor)?;
            surface.set_input_region(Some(region.wl_region()));
            let viewport = self
                .viewporter
                .get()
                .unwrap()
                .get_viewport(&surface, qh, ());
            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("omarchy-cursor"),
                Some(&output),
            );
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_anchor(Anchor::TOP | Anchor::LEFT);
            layer.set_exclusive_zone(-1);
            layer.set_size(1, 1);
            layer.commit();
            self.layers.push(Overlay {
                output,
                monitor: m,
                layer,
                viewport,
                ready: false,
                visible: false,
                pool: SlotPool::new(1024 * 1024, &self.shm)?,
            });
            self.dirty = true;
        }
        Ok(())
    }
    fn hide(&mut self) -> Result<()> {
        for l in &mut self.layers {
            l.hide()?;
        }
        Ok(())
    }
    fn draw(&mut self, _qh: &QueueHandle<Self>) -> Result<()> {
        let idle = self.last_move.elapsed().as_secs_f32();
        let locating = self
            .locate
            .map(|t| t.elapsed().as_secs_f32())
            .filter(|t| *t < 1.5);
        if locating.is_none() {
            self.locate = None;
        }
        let attention = locating.or_else(|| {
            (self.config.attention && idle > self.config.attention_after).then_some(idle)
        });
        let opacity = if attention.is_some() {
            1.
        } else if self.config.auto_hide {
            (1. - (idle - self.config.hide_after) * 4.).clamp(0., 1.)
        } else {
            1.
        };
        let pulse = attention.map_or(0., |t| (t * 8.).sin().abs());
        let c = &self.config;
        for l in &mut self.layers {
            let visible = l.monitor.contains(self.cursor.0, self.cursor.1)
                && (c.enabled || locating.is_some() || self.magnify)
                && (opacity > 0. || self.magnify);
            if !visible {
                l.hide()?;
                continue;
            }
            if !l.visible && !l.ready {
                l.layer.commit();
                l.visible = true;
                continue;
            }
            if !l.ready {
                continue;
            }
            let local = (
                self.cursor.0 - l.monitor.x as f32,
                self.cursor.1 - l.monitor.y as f32,
            );
            let screen = l.monitor.size();
            let lens_center = render::lens_center(local, c.lens_size, c.zoom, screen);
            let margin = (c.radius * 2. + 30.).max(80.);
            let (left, top, right, bottom) = if self.magnify {
                let r = c.lens_size / 2. + 16.;
                (
                    local.0.min(lens_center.0 - r) - 8.,
                    local.1.min(lens_center.1 - r) - 8.,
                    local.0.max(lens_center.0 + r) + 8.,
                    local.1.max(lens_center.1 + r) + 8.,
                )
            } else {
                (
                    local.0 - margin,
                    local.1 - margin,
                    local.0 + margin,
                    local.1 + margin,
                )
            };
            let left = left.floor().max(0.) as i32;
            let top = top.floor().max(0.) as i32;
            let width = (right.ceil().min(screen.0 as f32) as i32 - left).max(1);
            let height = (bottom.ceil().min(screen.1 as f32) as i32 - top).max(1);
            let scale = l.monitor.scale;
            let pw = (width as f32 * scale).ceil() as u32;
            let ph = (height as f32 * scale).ceil() as u32;
            let mut pix = tiny_skia::Pixmap::new(pw, ph).context("Invalid overlay dimensions")?;
            if self.magnify {
                if self.capture_monitor == l.monitor.name
                    && let Some(image) = self.capture.as_ref().and_then(|c| c.image.as_ref())
                {
                    render::lens(
                        &mut pix,
                        image,
                        (lens_center.0 - left as f32, lens_center.1 - top as f32),
                        c,
                        scale,
                    );
                }
            } else {
                let age = self.last_click.elapsed().as_secs_f32();
                render::highlight(
                    &mut pix,
                    c,
                    &render::Highlight {
                        center: (local.0 - left as f32, local.1 - top as f32),
                        opacity,
                        pulse,
                        click_age: age,
                        button: if self.held || age < 0.4 {
                            self.button
                        } else {
                            None
                        },
                        held: self.held,
                        velocity: self.velocity,
                    },
                    scale,
                );
            }
            l.layer.set_margin(top, 0, 0, left);
            l.layer.set_size(width as u32, height as u32);
            l.viewport.set_destination(width, height);
            let (buffer, canvas) = l.pool.create_buffer(
                pw as i32,
                ph as i32,
                pw as i32 * 4,
                wl_shm::Format::Argb8888,
            )?;
            for (source, target) in pix
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(canvas.as_chunks_mut::<4>().0.iter_mut())
            {
                target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            }
            l.layer
                .wl_surface()
                .damage_buffer(0, 0, pw as i32, ph as i32);
            buffer.attach_to(l.layer.wl_surface())?;
            l.layer.commit();
            l.visible = true;
        }
        Ok(())
    }
    fn request_capture(&mut self, qh: &QueueHandle<Self>) {
        let Some(c) = self.capture.as_mut() else {
            return;
        };
        if !self.magnify {
            if c.frame.is_some() || c.image.is_some() {
                c.cancel();
            }
            return;
        }
        if c.frame.is_some() && self.last_capture.elapsed() > Duration::from_secs(2) {
            c.cancel();
            c.error = Some("Screen capture timed out".into());
        }
        if c.frame.is_some() || self.last_capture.elapsed() < Duration::from_millis(33) {
            return;
        }
        let Some(l) = self
            .layers
            .iter()
            .find(|l| l.monitor.contains(self.cursor.0, self.cursor.1))
        else {
            return;
        };
        let size = (self.config.lens_size / self.config.zoom).ceil() as i32;
        let screen = l.monitor.size();
        let source_x = (self.cursor.0 - l.monitor.x as f32) as i32 - size / 2;
        let source_y = (self.cursor.1 - l.monitor.y as f32) as i32 - size / 2;
        let x = source_x.max(0);
        let y = source_y.max(0);
        let width = (source_x + size).min(screen.0) - x;
        let height = (source_y + size).min(screen.1) - y;
        c.source_size = size as f32;
        c.source_width = width as f32;
        c.source_offset = ((x - source_x) as f32, (y - source_y) as f32);
        if self.capture_monitor != l.monitor.name {
            c.image = None;
        }
        self.capture_monitor = l.monitor.name.clone();
        c.frame = Some(
            c.manager
                .capture_output_region(0, &l.output, x, y, width, height, qh, ()),
        );
        self.last_capture = Instant::now();
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
        self.refresh = true;
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
        self.refresh = true;
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
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
impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh = true;
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh = true;
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh = true;
    }
}
impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.layers.retain(|l| l.layer != *layer);
        self.refresh = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.layer == *layer)
            && !l.ready
        {
            l.ready = true;
            self.dirty = true;
        }
    }
}
impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}
impl Dispatch<WpViewport, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: wp_viewport::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_compositor!(State);
delegate_output!(State);
delegate_shm!(State);
delegate_layer!(State);
delegate_registry!(State);
delegate_simple!(State, WpViewporter, 1);

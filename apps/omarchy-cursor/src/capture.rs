//! Region capture through the protocol supported by the installed Hyprland.
//! Frames are kept in memory and never written to disk.
use crate::overlay::State;
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, SlotPool},
};
use tiny_skia::Pixmap;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, protocol::wl_shm};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self as frame, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self as manager, ZwlrScreencopyManagerV1},
};

pub struct Capture {
    pub manager: ZwlrScreencopyManagerV1,
    pub frame: Option<ZwlrScreencopyFrameV1>,
    pub image: Option<Pixmap>,
    pub error: Option<String>,
    pub frames: u64,
    pub source_size: f32,
    pub source_width: f32,
    pub source_offset: (f32, f32),
    pool: SlotPool,
    buffer: Option<Buffer>,
    width: u32,
    height: u32,
    stride: u32,
    format: wl_shm::Format,
    invert: bool,
}
impl Capture {
    pub fn new(manager: ZwlrScreencopyManagerV1, shm: &Shm) -> anyhow::Result<Self> {
        Ok(Self {
            manager,
            frame: None,
            image: None,
            error: None,
            frames: 0,
            source_size: 1.,
            source_width: 1.,
            source_offset: (0., 0.),
            pool: SlotPool::new(1024 * 1024, shm)?,
            buffer: None,
            width: 0,
            height: 0,
            stride: 0,
            format: wl_shm::Format::Xrgb8888,
            invert: false,
        })
    }
    pub fn cancel(&mut self) {
        if let Some(f) = self.frame.take() {
            f.destroy();
        }
        self.buffer = None;
        self.image = None;
    }
    fn finish(&mut self) {
        if let Some(f) = self.frame.take() {
            f.destroy();
        }
        self.buffer = None;
    }
}
impl Dispatch<ZwlrScreencopyManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwlrScreencopyFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrScreencopyFrameV1,
        event: frame::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(c) = state.capture.as_mut() else {
            return;
        };
        if c.frame.as_ref() != Some(proxy) {
            return;
        }
        match event {
            frame::Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                if !matches!(
                    format,
                    wl_shm::Format::Xrgb8888
                        | wl_shm::Format::Argb8888
                        | wl_shm::Format::Xbgr8888
                        | wl_shm::Format::Abgr8888
                ) || width > 4096
                    || height > 4096
                {
                    c.error = Some(format!(
                        "Unsupported capture format or size: {format:?} {width}×{height}"
                    ));
                    c.finish();
                    return;
                }
                c.width = width;
                c.height = height;
                c.stride = stride;
                c.format = format;
                c.invert = false;
                match c
                    .pool
                    .create_buffer(width as i32, height as i32, stride as i32, format)
                {
                    Ok((buffer, _)) => c.buffer = Some(buffer),
                    Err(e) => {
                        c.error = Some(e.to_string());
                        c.finish();
                    }
                }
            }
            frame::Event::BufferDone => {
                if let Some(b) = &c.buffer {
                    proxy.copy(b.wl_buffer());
                } else {
                    c.error = Some("Compositor did not offer a supported capture buffer".into());
                    c.finish();
                }
            }
            frame::Event::Flags {
                flags: WEnum::Value(flags),
            } => c.invert = flags.contains(frame::Flags::YInvert),
            frame::Event::Ready { .. } => {
                let scale = c.width as f32 / c.source_width;
                let size = (c.source_size * scale).round().max(1.) as u32;
                let offset_x = (c.source_offset.0 * scale).round() as u32;
                let offset_y = (c.source_offset.1 * scale).round() as u32;
                if let Some(buffer) = &c.buffer
                    && let Some(data) = c.pool.canvas(&buffer.slot())
                    && let Some(mut pix) = Pixmap::new(size, size)
                {
                    pix.fill(tiny_skia::Color::from_rgba8(20, 21, 28, 255));
                    let bgr = matches!(
                        c.format,
                        wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888
                    );
                    for y in 0..c.height as usize {
                        let source_y = if c.invert {
                            c.height as usize - 1 - y
                        } else {
                            y
                        };
                        for x in 0..c.width as usize {
                            let s = source_y * c.stride as usize + x * 4;
                            let dx = x + offset_x as usize;
                            let dy = y + offset_y as usize;
                            if dx >= size as usize || dy >= size as usize {
                                continue;
                            }
                            let d = (dy * size as usize + dx) * 4;
                            let (r, g, b) = if bgr {
                                (data[s + 2], data[s + 1], data[s])
                            } else {
                                (data[s], data[s + 1], data[s + 2])
                            };
                            pix.data_mut()[d..d + 4].copy_from_slice(&[r, g, b, 255]);
                        }
                    }
                    c.image = Some(pix);
                    c.frames += 1;
                    c.error = None;
                }
                c.finish();
                state.dirty = true;
            }
            frame::Event::Failed => {
                c.error =
                    Some("Screen capture failed; check Hyprland screen-capture permissions".into());
                c.finish();
            }
            _ => {}
        }
    }
}

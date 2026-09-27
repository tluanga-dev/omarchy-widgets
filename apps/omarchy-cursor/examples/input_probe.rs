//! Manual integration probe: inject one click through Wayland (never reads input).
//! Usage: cargo run --release --example input_probe -- [left|right] [hold_ms]
use wayland_client::{
    Connection, Dispatch, QueueHandle, globals::registry_queue_init,
    protocol::wl_pointer::ButtonState,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::{self as manager, ZwlrVirtualPointerManagerV1},
    zwlr_virtual_pointer_v1::{self as pointer, ZwlrVirtualPointerV1},
};
struct Probe;
impl
    Dispatch<
        wayland_client::protocol::wl_registry::WlRegistry,
        wayland_client::globals::GlobalListContents,
    > for Probe
{
    fn event(
        _: &mut Self,
        _: &wayland_client::protocol::wl_registry::WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for Probe {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwlrVirtualPointerV1, ()> for Probe {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let button = if args.get(1).map(String::as_str) == Some("right") {
        273
    } else {
        272
    };
    let ms = args
        .get(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(150)
        .min(3000);
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Probe>(&conn)?;
    let qh = queue.handle();
    let manager: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=2, ())?;
    let pointer = manager.create_virtual_pointer(None, &qh, ());
    queue.roundtrip(&mut Probe)?;
    pointer.button(1, button, ButtonState::Pressed);
    pointer.frame();
    conn.flush()?;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    pointer.button(ms as u32 + 1, button, ButtonState::Released);
    pointer.frame();
    queue.roundtrip(&mut Probe)?;
    pointer.destroy();
    manager.destroy();
    conn.flush()?;
    Ok(())
}

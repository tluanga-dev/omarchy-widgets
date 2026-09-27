//! Manual integration probe for Super+Alt+C/M/L using a standard evdev keymap.
//! wtype's synthetic keycodes do not match Hyprland's default physical bind map.
use smithay_client_toolkit::reexports::protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use std::{
    io::Write,
    os::{fd::AsFd, unix::fs::OpenOptionsExt},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_seat},
};
struct Probe;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Probe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(Probe: ignore wl_seat::WlSeat);
delegate_noop!(Probe: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(Probe: ignore ZwpVirtualKeyboardV1);
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("c") => 46,
        Some("m") => 50,
        Some("l") => 38,
        _ => anyhow::bail!("Use c, m, or l"),
    };
    let duration = args
        .get(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(150)
        .min(3000);
    let map = std::process::Command::new("xkbcli")
        .args(["compile-keymap", "--layout", "us"])
        .output()?;
    anyhow::ensure!(map.status.success(), "xkbcli failed");
    let path = std::env::temp_dir().join(format!("omarchy-cursor-keymap-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&path)?;
    std::fs::remove_file(path)?;
    file.write_all(&map.stdout)?;
    file.write_all(&[0])?;
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Probe>(&conn)?;
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    keyboard.keymap(1, file.as_fd(), map.stdout.len() as u32 + 1);
    queue.roundtrip(&mut Probe)?;
    keyboard.modifiers(72, 0, 0, 0);
    keyboard.key(1, code, 1);
    conn.flush()?;
    std::thread::sleep(std::time::Duration::from_millis(duration));
    keyboard.key(duration as u32 + 1, code, 0);
    keyboard.modifiers(0, 0, 0, 0);
    queue.roundtrip(&mut Probe)?;
    keyboard.destroy();
    conn.flush()?;
    Ok(())
}

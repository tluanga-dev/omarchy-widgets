use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

#[derive(Clone, Debug, Deserialize)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub transform: u32,
}
impl Monitor {
    pub fn size(&self) -> (i32, i32) {
        let (w, h) = if self.transform % 2 == 1 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        (
            (w as f32 / self.scale).round() as i32,
            (h as f32 / self.scale).round() as i32,
        )
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let (w, h) = self.size();
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + w) as f32
            && y < (self.y + h) as f32
    }
}
pub fn runtime() -> Result<PathBuf> {
    Ok(std::env::var_os("XDG_RUNTIME_DIR")
        .context("Run this app from your Omarchy desktop session (XDG_RUNTIME_DIR missing)")?
        .into())
}
pub fn request(command: &str) -> Result<String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("Hyprland session is not available")?;
    let mut s = UnixStream::connect(runtime()?.join("hypr").join(sig).join(".socket.sock"))?;
    s.set_read_timeout(Some(Duration::from_millis(300)))?;
    s.set_write_timeout(Some(Duration::from_millis(300)))?;
    s.write_all(command.as_bytes())?;
    let mut data = String::new();
    s.take(2 * 1024 * 1024).read_to_string(&mut data)?;
    Ok(data)
}
pub fn cursor() -> Result<(f32, f32)> {
    #[derive(Deserialize)]
    struct Position {
        x: f32,
        y: f32,
    }
    let p: Position = serde_json::from_str(&request("j/cursorpos")?)?;
    Ok((p.x, p.y))
}
pub fn monitors() -> Result<Vec<Monitor>> {
    Ok(serde_json::from_str(&request("j/monitors")?)?)
}
pub fn doctor() -> Result<()> {
    ensure!(
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        "WAYLAND_DISPLAY is missing"
    );
    println!("Hyprland cursor position: {:?}", cursor()?);
    for m in monitors()? {
        println!(
            "{}: {:?} logical pixels at {}× scale",
            m.name,
            m.size(),
            m.scale
        );
    }
    println!("Preferences: {}", crate::config::dir()?.display());
    println!(
        "Background app: {}",
        crate::ipc::send("status").unwrap_or_else(|_| "stopped".into())
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_rotated_negative_origin() {
        let mut m = Monitor {
            name: "test".into(),
            x: -1152,
            y: 0,
            width: 2560,
            height: 1440,
            scale: 1.25,
            transform: 1,
        };
        assert_eq!(m.size(), (1152, 2048));
        assert!(m.contains(-1., 2000.));
        assert!(!m.contains(0., 2000.));
        m.transform = 0;
        assert_eq!(m.size(), (2048, 1152));
    }
}

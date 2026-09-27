use anyhow::{Context, Result, bail};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
pub fn dir() -> Result<PathBuf> {
    let path = crate::hypr::runtime()?.join("omarchy-cursor");
    fs::create_dir_all(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}
pub fn send(message: &str) -> Result<String> {
    let mut stream = UnixStream::connect(dir()?.join("control.sock"))
        .context("Omarchy Cursor is not running. Launch omarchy-cursor first.")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(message.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut reply = String::new();
    stream.take(16384).read_to_string(&mut reply)?;
    Ok(reply)
}
pub fn listen() -> Result<(std::fs::File, UnixListener)> {
    let dir = dir()?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("daemon.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).context("Omarchy Cursor is already running")?;
    let path = dir.join("control.sock");
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(path)?;
    listener.set_nonblocking(true)?;
    Ok((lock, listener))
}
pub fn ensure_daemon() -> Result<()> {
    if send("status").is_ok() {
        return Ok(());
    }
    if crate::install::service_path()?.exists() {
        for attempt in 0..60 {
            // The socket can close just before systemd observes a clean exit.
            // A start in that interval is a no-op; retry once the old unit exits.
            if attempt % 10 == 0 {
                let result = Command::new("systemctl")
                    .args(["--user", "start", "omarchy-cursor.service"])
                    .output()?;
                if !result.status.success() {
                    bail!(
                        "Could not start background service: {}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
            }
            if send("status").is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        bail!("Background app did not start. Inspect journalctl --user -u omarchy-cursor.service");
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir()?.join("daemon.log"))?;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    for _ in 0..60 {
        if send("status").is_ok() {
            return Ok(());
        }
        if let Some(code) = child.try_wait()? {
            bail!(
                "Background app exited ({code}). See {}/daemon.log",
                dir()?.display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "Background app did not start. See {}/daemon.log",
        dir()?.display()
    )
}

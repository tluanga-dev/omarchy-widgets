mod capture;
mod config;
mod hypr;
mod install;
mod ipc;
mod overlay;
mod render;
mod settings;
mod tray;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open preferences and start the background app if necessary.
    Settings,
    /// Run the background overlay and tray icon.
    Daemon,
    /// Start the background app without opening preferences.
    Start,
    /// Enable or disable highlighting.
    Toggle,
    /// Toggle the magnifying lens.
    Magnify,
    /// Show the magnifier while a shortcut is held.
    #[command(hide = true)]
    MagnifyOn,
    /// Hide the magnifier when the shortcut is released.
    #[command(hide = true)]
    MagnifyOff,
    /// Pulse the highlight to locate the pointer.
    Locate,
    /// Report background app state as JSON.
    Status,
    /// Reload saved preferences.
    Reload,
    /// Stop the background app.
    Quit,
    /// Used by non-consuming Hyprland mouse bindings.
    Click { button: String, state: String },
    /// Install the app, launcher entry, and Hyprland shortcuts for this user.
    Install {
        /// Use the Quickshell bar widget instead of the standalone tray icon.
        #[arg(long)]
        widget: bool,
    },
    /// Remove integration and the installed binary; keep preferences.
    Uninstall,
    /// Check the compositor and capture capabilities.
    Doctor,
}

fn main() -> Result<()> {
    match Cli::parse().command.unwrap_or(Command::Settings) {
        Command::Settings => {
            ipc::ensure_daemon()?;
            settings::run()
        }
        Command::Daemon => overlay::run(),
        Command::Start => ipc::ensure_daemon(),
        Command::Install { widget } => {
            install::install()?;
            if widget {
                let mut c = config::Config::load()?;
                c.show_tray = false;
                c.save()?;
                let _ = ipc::send("reload");
            }
            Ok(())
        }
        Command::Uninstall => install::uninstall(),
        Command::Doctor => hypr::doctor(),
        Command::Click { button, state } => {
            // Mouse bindings must remain silent and harmless when the app is stopped.
            if ["left", "right", "middle"].contains(&button.as_str())
                && ["down", "up"].contains(&state.as_str())
            {
                let _ = ipc::send(&format!("click {button} {state}"));
            }
            Ok(())
        }
        command => {
            let message = match command {
                Command::Toggle => "toggle",
                Command::Magnify => "magnify",
                Command::MagnifyOn => "magnify-on",
                Command::MagnifyOff => "magnify-off",
                Command::Locate => "locate",
                Command::Status => "status",
                Command::Reload => "reload",
                Command::Quit => "quit",
                _ => unreachable!(),
            };
            println!("{}", ipc::send(message)?);
            Ok(())
        }
    }
}

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Circle,
    Squircle,
    Rhombus,
    Rectangle,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Border {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub show_tray: bool,
    pub shape: Shape,
    pub radius: f32,
    pub weight: f32,
    pub border: Border,
    pub glow: f32,
    pub color: [u8; 4],
    pub left_color: [u8; 4],
    pub right_color: [u8; 4],
    pub animate: bool,
    pub perspective: bool,
    pub auto_hide: bool,
    pub hide_after: f32,
    pub attention: bool,
    pub attention_after: f32,
    pub magnifier_enabled: bool,
    pub magnify_hold: bool,
    pub zoom: f32,
    pub lens_size: f32,
    pub smooth: bool,
    pub toggle_key: String,
    pub magnify_key: String,
    pub locate_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            show_tray: true,
            shape: Shape::Circle,
            radius: 24.0,
            weight: 3.0,
            border: Border::Solid,
            glow: 0.35,
            color: [163, 119, 255, 255],
            left_color: [99, 224, 181, 255],
            right_color: [255, 157, 100, 255],
            animate: true,
            perspective: true,
            auto_hide: false,
            hide_after: 2.5,
            attention: false,
            attention_after: 8.0,
            magnifier_enabled: true,
            magnify_hold: false,
            zoom: 2.5,
            lens_size: 220.0,
            smooth: true,
            toggle_key: "SUPER + ALT + C".into(),
            magnify_key: "SUPER + ALT + M".into(),
            locate_key: "SUPER + ALT + L".into(),
        }
    }
}

pub fn home() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME").context("HOME is not set")?.into())
}
pub fn dir() -> Result<PathBuf> {
    Ok(std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".config"))
        .join("omarchy-cursor"))
}
impl Config {
    pub fn load() -> Result<Self> {
        let path = dir()?.join("config.json");
        let config: Self = match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data)
                .with_context(|| format!("Invalid preferences: {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        for (name, value, low, high) in [
            ("radius", self.radius, 10., 100.),
            ("weight", self.weight, 1., 12.),
            ("glow", self.glow, 0., 1.),
            ("hide_after", self.hide_after, 0.3, 30.),
            ("attention_after", self.attention_after, 2., 60.),
            ("zoom", self.zoom, 1.5, 10.),
            ("lens_size", self.lens_size, 120., 360.),
        ] {
            ensure!(
                value.is_finite() && (low..=high).contains(&value),
                "{name} must be between {low} and {high}"
            );
        }
        for key in [&self.toggle_key, &self.magnify_key, &self.locate_key] {
            ensure!(
                !key.is_empty()
                    && key.len() < 100
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || " +_".contains(c)),
                "Invalid shortcut: {key}"
            );
            ensure!(
                key.contains("SUPER") || key.contains("CTRL") || key.contains("ALT"),
                "Shortcut must include SUPER, CTRL, or ALT"
            );
        }
        ensure!(
            self.toggle_key != self.magnify_key
                && self.toggle_key != self.locate_key
                && self.magnify_key != self.locate_key,
            "Shortcuts must be different"
        );
        Ok(())
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let dir = dir()?;
        fs::create_dir_all(&dir)?;
        let temp = dir.join(format!("config.{}.tmp", std::process::id()));
        fs::write(&temp, serde_json::to_string_pretty(self)? + "\n")?;
        fs::rename(temp, dir.join("config.json"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_settings_use_defaults() {
        let c: Config = serde_json::from_str(r#"{"zoom":3.0}"#).unwrap();
        assert_eq!(c.zoom, 3.);
        assert_eq!(c.radius, 24.);
        c.validate().unwrap();
    }
    #[test]
    fn unsafe_or_invalid_config_is_rejected() {
        let mut c = Config {
            zoom: f32::NAN,
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.zoom = 2.;
        c.toggle_key = "SUPER + C\nexecute()".into();
        assert!(c.validate().is_err());
        c.toggle_key = c.magnify_key.clone();
        assert!(c.validate().is_err());
    }
}

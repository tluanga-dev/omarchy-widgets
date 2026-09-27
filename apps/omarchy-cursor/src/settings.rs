use crate::{
    config::{Border, Config, Shape},
    install, ipc, render,
};
use anyhow::Result;
use eframe::egui::{self, Color32, RichText, Vec2};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Appearance,
    Magnifier,
    Behavior,
}
struct Preferences {
    config: Config,
    saved: Config,
    tab: Tab,
    message: String,
    error: bool,
    preview: Option<egui::TextureHandle>,
    click: Instant,
    right: bool,
    start: Instant,
    autostart: bool,
    status: String,
    last_status: Instant,
}

pub fn run() -> Result<()> {
    let c = Config::load()?;
    // A second launcher activation focuses the existing settings window.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(ipc::dir()?.join("settings.lock"))?;
    if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
        let _ = std::process::Command::new("hyprctl")
            .args([
                "dispatch",
                "hl.dsp.focus({window=\"class:^(omarchy-cursor)$\"})",
            ])
            .output();
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Omarchy Cursor — Preferences")
            .with_app_id("omarchy-cursor")
            .with_inner_size([640., 900.])
            .with_min_inner_size([560., 650.]),
        ..Default::default()
    };
    eframe::run_native(
        "Omarchy Cursor",
        options,
        Box::new(move |cc| {
            let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = Color32::from_rgb(30, 31, 39);
            style.visuals.window_fill = Color32::from_rgb(30, 31, 39);
            style.visuals.selection.bg_fill = Color32::from_rgb(100, 72, 158);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(100, 72, 158);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(49, 50, 62);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(64, 61, 82);
            style.spacing.item_spacing = Vec2::new(12., 12.);
            style.spacing.button_padding = Vec2::new(14., 8.);
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(15.));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(15.));
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
            Ok(Box::new(Preferences {
                saved: c.clone(),
                config: c,
                tab: Tab::Appearance,
                message: "Changes apply instantly".into(),
                error: false,
                preview: None,
                click: Instant::now() - Duration::from_secs(60),
                right: false,
                start: Instant::now(),
                autostart: install::autostart_enabled(),
                status: "Connecting…".into(),
                last_status: Instant::now() - Duration::from_secs(10),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Preferences window: {e}"))
}

impl Preferences {
    fn command(&mut self, cmd: &str) {
        match ipc::send(cmd) {
            Ok(s) => {
                self.error = s.starts_with("error:");
                self.message = if self.error { s } else { "Applied".into() };
            }
            Err(e) => {
                self.error = true;
                self.message = e.to_string();
            }
        }
    }
    fn preview(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let height = 190.;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
        if response.clicked() || response.secondary_clicked() {
            self.click = Instant::now();
            self.right = response.secondary_clicked();
        }
        let paint = ui.painter_at(rect);
        paint.rect_filled(rect, 10, Color32::from_rgb(20, 21, 28));
        for x in (0..width as i32).step_by(14) {
            for y in (0..height as i32).step_by(14) {
                paint.circle_filled(
                    rect.min + Vec2::new(x as f32, y as f32),
                    0.7,
                    Color32::from_rgb(42, 43, 54),
                );
            }
        }
        let center = (width / 2., height / 2.);
        let mut pix = tiny_skia::Pixmap::new((width * 2.) as u32, (height * 2.) as u32).unwrap();
        let age = self.click.elapsed().as_secs_f32();
        if self.tab == Tab::Magnifier {
            // A sample grid demonstrates magnifier shape and smoothing, without screen capture.
            let mut sample = tiny_skia::Pixmap::new(64, 64).unwrap();
            for (i, p) in sample
                .data_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                let x = i % 64;
                let y = i / 64;
                let color = if x % 8 == 0 || y % 8 == 0 {
                    [110, 100, 140, 255]
                } else {
                    [35, 34, 48, 255]
                };
                p.copy_from_slice(&color);
            }
            let mut c = self.config.clone();
            c.lens_size = c.lens_size.min(174.);
            render::lens(&mut pix, &sample, center, &c, 2.);
        } else {
            render::highlight(
                &mut pix,
                &self.config,
                &render::Highlight {
                    center,
                    opacity: 1.,
                    pulse: if self.tab == Tab::Behavior && self.config.attention {
                        (self.start.elapsed().as_secs_f32() * 4.).sin().abs()
                    } else {
                        0.
                    },
                    click_age: age,
                    button: if age < 0.5 { Some(!self.right) } else { None },
                    held: response.is_pointer_button_down_on(),
                    velocity: if self.config.perspective {
                        (30. * self.start.elapsed().as_secs_f32().sin(), 0.)
                    } else {
                        (0., 0.)
                    },
                },
                2.,
            );
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [pix.width() as usize, pix.height() as usize],
            pix.data(),
        );
        if let Some(texture) = &mut self.preview {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.preview = Some(ui.ctx().load_texture(
                "preview",
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
        paint.image(
            self.preview.as_ref().unwrap().id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
        let p = rect.min + Vec2::new(center.0, center.1);
        paint.add(egui::Shape::convex_polygon(
            vec![
                p + Vec2::new(-4., -10.),
                p + Vec2::new(-4., 12.),
                p + Vec2::new(2., 6.),
                p + Vec2::new(7., 16.),
                p + Vec2::new(11., 14.),
                p + Vec2::new(6., 4.),
                p + Vec2::new(14., 4.),
            ],
            Color32::WHITE,
            egui::Stroke::new(1.5, Color32::from_rgb(15, 15, 20)),
        ));
        paint.text(
            rect.center_bottom() - Vec2::new(0., 15.),
            egui::Align2::CENTER_BOTTOM,
            if self.tab == Tab::Magnifier {
                "Live magnification is available with your shortcut"
            } else {
                "Click or right-click to preview"
            },
            egui::FontId::proportional(12.),
            Color32::from_rgb(145, 146, 165),
        );
    }
    fn appearance(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("appearance")
            .num_columns(2)
            .spacing([24., 14.])
            .show(ui, |ui| {
                ui.label("Shape");
                egui::ComboBox::from_id_salt("shape")
                    .selected_text(format!("{:?}", self.config.shape))
                    .width(245.)
                    .show_ui(ui, |ui| {
                        for (v, label) in [
                            (Shape::Circle, "Circle"),
                            (Shape::Squircle, "Squircle"),
                            (Shape::Rhombus, "Rhombus"),
                            (Shape::Rectangle, "Rectangle"),
                        ] {
                            ui.selectable_value(&mut self.config.shape, v, label);
                        }
                    });
                ui.end_row();
                ui.label("Size");
                ui.add(
                    egui::Slider::new(&mut self.config.radius, 10.0..=100.0)
                        .suffix(" px")
                        .text("radius"),
                );
                ui.end_row();
                ui.label("Border weight");
                ui.add(egui::Slider::new(&mut self.config.weight, 1.0..=12.0).suffix(" px"));
                ui.end_row();
                ui.label("Border style");
                ui.horizontal(|ui| {
                    for v in [Border::Solid, Border::Dashed, Border::Dotted] {
                        ui.selectable_value(&mut self.config.border, v, format!("{v:?}"));
                    }
                });
                ui.end_row();
                ui.label("Glow");
                ui.add(egui::Slider::new(&mut self.config.glow, 0.0..=1.0));
                ui.end_row();
                ui.label("Clicks");
                ui.checkbox(&mut self.config.animate, "Animate clicks and releases");
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut self.config.perspective, "Perspective warping");
                ui.end_row();
                for (label, color) in [
                    ("Highlight", &mut self.config.color),
                    ("Left click", &mut self.config.left_color),
                    ("Right click", &mut self.config.right_color),
                ] {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgba_unmultiplied(color);
                        ui.label(
                            RichText::new(format!(
                                "#{:02X}{:02X}{:02X}",
                                color[0], color[1], color[2]
                            ))
                            .monospace()
                            .weak(),
                        );
                    });
                    ui.end_row();
                }
            });
    }
    fn magnifier(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.config.magnifier_enabled, "Enable magnifier");
        ui.add_space(8.);
        ui.add_enabled_ui(self.config.magnifier_enabled, |ui| {
            egui::Grid::new("magnifier")
                .num_columns(2)
                .spacing([24., 18.])
                .show(ui, |ui| {
                    ui.label("Magnifier size");
                    ui.add(
                        egui::Slider::new(&mut self.config.lens_size, 120.0..=360.0).suffix(" px"),
                    );
                    ui.end_row();
                    ui.label("Zoom factor");
                    ui.add(egui::Slider::new(&mut self.config.zoom, 1.5..=10.0).suffix("×"));
                    ui.end_row();
                    ui.label("Quality");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.config.smooth, true, "Smooth");
                        ui.selectable_value(&mut self.config.smooth, false, "Crisp pixels");
                    });
                    ui.end_row();
                    ui.label("Shortcut");
                    ui.label(RichText::new(&self.config.magnify_key).monospace());
                    ui.end_row();
                });
            ui.add_space(20.);
            if ui.button("Toggle live magnifier").clicked() {
                self.command("magnify");
            }
        });
        ui.add_space(18.);
        ui.separator();
        ui.add_space(8.);
        ui.label(RichText::new("A closer look, always within reach").strong());
        ui.label("The lens follows beside your pointer. Press the shortcut again to dismiss it. Screen pixels stay in memory on this computer.");
    }
    fn behavior(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.config.auto_hide,
            "Hide highlight while the pointer is idle",
        );
        ui.add_enabled(
            self.config.auto_hide,
            egui::Slider::new(&mut self.config.hide_after, 0.3..=30.)
                .suffix(" s")
                .text("Hide after"),
        );
        ui.add_space(6.);
        ui.checkbox(
            &mut self.config.attention,
            "Pulse to attract attention when idle",
        );
        ui.add_enabled(
            self.config.attention,
            egui::Slider::new(&mut self.config.attention_after, 2.0..=60.0)
                .suffix(" s")
                .text("Pulse after"),
        );
        ui.add_space(6.);
        if ui
            .add_enabled(
                install::installed(),
                egui::Checkbox::new(&mut self.autostart, "Launch at login"),
            )
            .changed()
            && let Err(e) = install::set_autostart(self.autostart)
        {
            self.error = true;
            self.message = e.to_string();
            self.autostart = install::autostart_enabled();
        }
        ui.separator();
        ui.label(RichText::new("Keyboard shortcuts").strong());
        ui.checkbox(
            &mut self.config.magnify_hold,
            "Hold the magnifier shortcut instead of toggling",
        );
        egui::Grid::new("shortcuts")
            .spacing([20., 10.])
            .show(ui, |ui| {
                for (label, key) in [
                    ("Toggle highlight", &mut self.config.toggle_key),
                    ("Magnifier", &mut self.config.magnify_key),
                    ("Locate pointer", &mut self.config.locate_key),
                ] {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(key).desired_width(260.));
                    ui.end_row();
                }
            });
        if ui.button("Apply shortcuts").clicked() {
            match install::update_bindings(&self.config).and_then(|()| self.config.save()) {
                Ok(()) => {
                    self.saved = self.config.clone();
                    self.command("reload");
                    self.message = "Shortcuts updated".into();
                    self.error = false;
                }
                Err(e) => {
                    self.message = e.to_string();
                    self.error = true;
                }
            }
        }
        ui.label(RichText::new("Use names such as SUPER + ALT + C. Existing shortcuts are checked before applying.").small().weak());
    }
}

impl eframe::App for Preferences {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        if self.last_status.elapsed() > Duration::from_secs(2) {
            self.status = match ipc::send("status")
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            {
                Some(s) => {
                    if let Some(enabled) = s["enabled"].as_bool() {
                        self.config.enabled = enabled;
                    }
                    if s["capture_error"].is_string() {
                        format!("Magnifier: {}", s["capture_error"].as_str().unwrap())
                    } else if s["enabled"] == true {
                        "Highlight active".into()
                    } else {
                        "Highlight paused".into()
                    }
                }
                None => "Background app stopped".into(),
            };
            self.last_status = Instant::now();
        }
        let before = self.config.clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 31, 39))
                    .inner_margin(24.),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Omarchy Cursor").size(25.).strong());
                        ui.label(
                            RichText::new("Make every movement clear.")
                                .color(Color32::from_rgb(158, 159, 181)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.checkbox(&mut self.config.enabled, "Enabled");
                    });
                });
                ui.add_space(14.);
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (Tab::Appearance, "Appearance"),
                        (Tab::Magnifier, "Magnifier"),
                        (Tab::Behavior, "Behavior"),
                    ] {
                        ui.selectable_value(&mut self.tab, tab, RichText::new(label).size(16.));
                    }
                });
                ui.add_space(8.);
                self.preview(ui);
                ui.add_space(20.);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - 74.).max(100.))
                    .show(ui, |ui| match self.tab {
                        Tab::Appearance => self.appearance(ui),
                        Tab::Magnifier => self.magnifier(ui),
                        Tab::Behavior => self.behavior(ui),
                    });
                ui.add_space(6.);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.status)
                            .small()
                            .color(Color32::from_rgb(169, 148, 222)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Locate").clicked() {
                            self.command("locate");
                        }
                        if ui.small_button("Reset appearance").clicked() {
                            let d = Config::default();
                            self.config.shape = d.shape;
                            self.config.radius = d.radius;
                            self.config.weight = d.weight;
                            self.config.border = d.border;
                            self.config.glow = d.glow;
                            self.config.color = d.color;
                            self.config.left_color = d.left_color;
                            self.config.right_color = d.right_color;
                        }
                    });
                });
                ui.label(RichText::new(&self.message).small().color(if self.error {
                    Color32::from_rgb(255, 150, 140)
                } else {
                    Color32::from_rgb(137, 139, 156)
                }));
            });
        // Keep partially typed shortcuts out of the saved config; apply them explicitly.
        if self.config != before {
            let pending = appearance_changes(&self.config, &self.saved);
            if pending != appearance_changes(&before, &self.saved) {
                match pending.save() {
                    Ok(()) => {
                        self.saved = pending;
                        self.command("reload");
                        if !self.error {
                            self.message = "Saved · changes apply instantly".into();
                        }
                    }
                    Err(e) => {
                        self.message = e.to_string();
                        self.error = true;
                    }
                }
            }
        }
    }
}

fn appearance_changes(draft: &Config, saved: &Config) -> Config {
    let mut result = draft.clone();
    result.toggle_key = saved.toggle_key.clone();
    result.magnify_key = saved.magnify_key.clone();
    result.locate_key = saved.locate_key.clone();
    result.magnify_hold = saved.magnify_hold;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_appearance_does_not_save_unapplied_shortcuts() {
        let saved = Config::default();
        let mut draft = saved.clone();
        draft.toggle_key = "SUPER +".into();
        draft.magnify_hold = !saved.magnify_hold;
        draft.radius = 42.;
        let persisted = appearance_changes(&draft, &saved);
        assert_eq!(persisted.radius, 42.);
        assert_eq!(persisted.toggle_key, saved.toggle_key);
        assert_eq!(persisted.magnify_hold, saved.magnify_hold);
        persisted.validate().unwrap();
    }
}

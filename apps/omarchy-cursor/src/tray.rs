use ksni::blocking::TrayMethods;

pub struct Tray {
    pub enabled: bool,
    pub magnifying: bool,
}
fn send(command: &str) {
    let _ = crate::ipc::send(command);
}
fn settings() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("settings").spawn();
    }
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "omarchy-cursor".into()
    }
    fn title(&self) -> String {
        "Omarchy Cursor".into()
    }
    fn icon_name(&self) -> String {
        "omarchy-cursor".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let mut pix = tiny_skia::Pixmap::new(32, 32).unwrap();
        let c = crate::config::Config {
            radius: 10.,
            weight: 2.5,
            color: if self.enabled {
                [185, 150, 255, 255]
            } else {
                [150, 150, 160, 255]
            },
            ..Default::default()
        };
        crate::render::highlight(
            &mut pix,
            &c,
            &crate::render::Highlight {
                center: (16., 16.),
                opacity: 1.,
                pulse: 0.,
                click_age: 10.,
                button: None,
                held: false,
                velocity: (0., 0.),
            },
            1.,
        );
        let data = pix
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect();
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data,
        }]
    }
    fn activate(&mut self, _: i32, _: i32) {
        settings();
    }
    fn secondary_activate(&mut self, _: i32, _: i32) {
        send("toggle");
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            CheckmarkItem {
                label: "Enable cursor highlight".into(),
                checked: self.enabled,
                activate: Box::new(|_| send("toggle")),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Magnifier".into(),
                checked: self.magnifying,
                activate: Box::new(|_| send("magnify")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Locate pointer".into(),
                activate: Box::new(|_| send("locate")),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Preferences…".into(),
                activate: Box::new(|_| settings()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit Omarchy Cursor".into(),
                activate: Box::new(|_| send("quit")),
                ..Default::default()
            }
            .into(),
        ]
    }
}
pub fn start() -> anyhow::Result<ksni::blocking::Handle<Tray>> {
    Ok(Tray {
        enabled: crate::config::Config::load()?.enabled,
        magnifying: false,
    }
    .spawn()?)
}

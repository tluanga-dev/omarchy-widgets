use crate::config::{Border, Config, Shape};
use tiny_skia::{
    FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

pub struct Highlight {
    pub center: (f32, f32),
    pub opacity: f32,
    pub pulse: f32,
    pub click_age: f32,
    pub button: Option<bool>,
    pub held: bool,
    pub velocity: (f32, f32),
}

pub fn shape_path(shape: Shape, x: f32, y: f32, r: f32) -> Path {
    match shape {
        Shape::Circle => PathBuilder::from_circle(x, y, r).unwrap(),
        Shape::Rectangle => {
            let mut p = PathBuilder::new();
            p.move_to(x - r, y - r * 0.72);
            p.line_to(x + r, y - r * 0.72);
            p.line_to(x + r, y + r * 0.72);
            p.line_to(x - r, y + r * 0.72);
            p.close();
            p.finish().unwrap()
        }
        _ => {
            // A superellipse gives both the rounded square and rounded diamond.
            let n = if shape == Shape::Squircle {
                4.0_f32
            } else {
                1.45_f32
            };
            let mut p = PathBuilder::new();
            for i in 0..=128 {
                let a = i as f32 / 128. * std::f32::consts::TAU;
                let px = x + r * a.cos().signum() * a.cos().abs().powf(2. / n);
                let py = y + r * a.sin().signum() * a.sin().abs().powf(2. / n);
                if i == 0 {
                    p.move_to(px, py)
                } else {
                    p.line_to(px, py)
                }
            }
            p.close();
            p.finish().unwrap()
        }
    }
}

fn paint(color: [u8; 4], opacity: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(
        color[0],
        color[1],
        color[2],
        (color[3] as f32 * opacity.clamp(0., 1.)) as u8,
    );
    p
}
pub fn highlight(pix: &mut Pixmap, config: &Config, h: &Highlight, scale: f32) {
    if h.opacity < 0.005 {
        return;
    }
    let color = match h.button {
        Some(true) => config.left_color,
        Some(false) => config.right_color,
        None => config.color,
    };
    let compress = if h.held && config.animate { 0.87 } else { 1. };
    let r = config.radius * compress * (1. + h.pulse * 0.32);
    let path = shape_path(config.shape, 0., 0., r);
    let v = if config.perspective {
        h.velocity
    } else {
        (0., 0.)
    };
    let sx = 1. + (v.0.abs() / 500.).min(0.12);
    let sy = 1. + (v.1.abs() / 500.).min(0.12);
    let ts = Transform::from_row(
        scale * sx,
        0.,
        0.,
        scale * sy,
        h.center.0 * scale,
        h.center.1 * scale,
    );
    for i in (1..=5).rev() {
        let stroke = Stroke {
            width: config.weight + i as f32 * 3.,
            ..Default::default()
        };
        pix.stroke_path(
            &path,
            &paint(color, h.opacity * config.glow * 0.07),
            &stroke,
            ts,
            None,
        );
    }
    let mut stroke = Stroke {
        width: config.weight,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    stroke.dash = match config.border {
        Border::Solid => None,
        Border::Dashed => StrokeDash::new(vec![config.weight * 2., config.weight * 2.], 0.),
        Border::Dotted => StrokeDash::new(vec![0.1, config.weight * 2.5], 0.),
    };
    // A faint dark outline keeps the highlight legible on light backgrounds.
    let outline = Stroke {
        width: config.weight + 1.5,
        dash: stroke.dash.clone(),
        ..stroke.clone()
    };
    pix.stroke_path(
        &path,
        &paint([12, 12, 20, 180], h.opacity),
        &outline,
        ts,
        None,
    );
    pix.stroke_path(&path, &paint(color, h.opacity), &stroke, ts, None);
    if config.animate && h.click_age < 0.45 {
        let t = (h.click_age / 0.45).clamp(0., 1.);
        let ripple = shape_path(config.shape, 0., 0., config.radius * (1. + 0.8 * t));
        pix.stroke_path(
            &ripple,
            &paint(color, (1. - t) * 0.6 * h.opacity),
            &Stroke {
                width: 1.5,
                ..Default::default()
            },
            ts,
            None,
        );
    }
}

/// The lens sits beside its source region so capture never feeds the lens back into itself.
pub fn lens_center(cursor: (f32, f32), size: f32, zoom: f32, screen: (i32, i32)) -> (f32, f32) {
    let r = size / 2.;
    let gap = r + r / zoom + 18.;
    let x = if cursor.0 + gap + r + 8. < screen.0 as f32 {
        cursor.0 + gap
    } else {
        cursor.0 - gap
    };
    (
        x.clamp(r + 8., (screen.0 as f32 - r - 8.).max(r + 8.)),
        cursor
            .1
            .clamp(r + 8., (screen.1 as f32 - r - 8.).max(r + 8.)),
    )
}
pub fn lens(pix: &mut Pixmap, image: &Pixmap, center: (f32, f32), c: &Config, scale: f32) {
    let radius = c.lens_size * 0.5;
    let path = shape_path(c.shape, center.0, center.1, radius);
    let ts = Transform::from_scale(scale, scale);
    let mut mask = tiny_skia::Mask::new(pix.width(), pix.height()).unwrap();
    mask.fill_path(&path, FillRule::Winding, true, ts);
    let factor = c.lens_size * scale / image.width() as f32;
    pix.draw_pixmap(
        0,
        0,
        image.as_ref(),
        &tiny_skia::PixmapPaint {
            quality: if c.smooth {
                tiny_skia::FilterQuality::Bilinear
            } else {
                tiny_skia::FilterQuality::Nearest
            },
            ..Default::default()
        },
        Transform::from_row(
            factor,
            0.,
            0.,
            factor,
            (center.0 - radius) * scale,
            (center.1 - radius) * scale,
        ),
        Some(&mask),
    );
    for i in (1..=5).rev() {
        pix.stroke_path(
            &path,
            &paint([0, 0, 0, 255], 0.06),
            &Stroke {
                width: 3. + i as f32 * 2.,
                ..Default::default()
            },
            ts,
            None,
        );
    }
    pix.stroke_path(
        &path,
        &paint(c.color, 1.),
        &Stroke {
            width: c.weight,
            ..Default::default()
        },
        ts,
        None,
    );
    let mut cross = PathBuilder::new();
    cross.move_to(center.0 - 5., center.1);
    cross.line_to(center.0 + 5., center.1);
    cross.move_to(center.0, center.1 - 5.);
    cross.line_to(center.0, center.1 + 5.);
    pix.stroke_path(
        &cross.finish().unwrap(),
        &paint(c.color, 0.85),
        &Stroke {
            width: 1.,
            ..Default::default()
        },
        ts,
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lens_stays_on_screen_and_away_from_source() {
        for x in [0., 50., 1024., 2000., 2047.] {
            let p = lens_center((x, 500.), 220., 2.5, (2048, 1152));
            assert!(p.0 >= 110. && p.0 + 110. <= 2048.);
            assert!((p.0 - x).abs() > 110. + 44.);
        }
    }
    #[test]
    fn rendering_leaves_center_transparent_and_draws_border() {
        let mut p = Pixmap::new(128, 128).unwrap();
        highlight(
            &mut p,
            &Config::default(),
            &Highlight {
                center: (64., 64.),
                opacity: 1.,
                pulse: 0.,
                click_age: 10.,
                button: None,
                held: false,
                velocity: (0., 0.),
            },
            1.,
        );
        assert_eq!(p.pixel(64, 64).unwrap().alpha(), 0);
        assert!(p.pixel(88, 64).unwrap().alpha() > 200);
        assert_eq!(p.pixel(0, 0).unwrap().alpha(), 0);
    }
}

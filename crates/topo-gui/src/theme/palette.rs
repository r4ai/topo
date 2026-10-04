use gpui::Rgba;

use super::Theme;

/// A color from `0xRRGGBB` and an alpha, with channels computed exactly as `gpui::rgb` does.
const fn hex(rgb: u32, a: f32) -> Rgba {
    Rgba {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a,
    }
}

pub static DARK: Theme = Theme {
    bg: hex(0x0f0f11, 1.0),
    glass_alpha: 0.78,
    chrome: hex(0xffffff, 0.035),
    card: hex(0x1a1a1d, 1.0),
    card_hover: hex(0x212125, 1.0),
    card_milestone: hex(0x222226, 1.0),
    card_milestone_hover: hex(0x2a2a2f, 1.0),
    overlay: hex(0x1e1e22, 0.97),
    control: hex(0xffffff, 0.06),
    control_hover: hex(0xffffff, 0.1),
    control_active: hex(0xffffff, 0.16),
    field: hex(0x000000, 0.25),
    hairline: hex(0xffffff, 0.08),
    border_strong: hex(0xffffff, 0.16),
    highlight: hex(0xffffff, 0.1),
    fg: hex(0xf2f2f3, 1.0),
    fg_muted: hex(0xa0a0a8, 1.0),
    fg_faint: hex(0x6a6a73, 1.0),
    emphasis: hex(0xffffff, 1.0),
    on_emphasis: hex(0x0f0f11, 1.0),
    selection: hex(0xffffff, 0.22),
    danger: hex(0xff6b6b, 1.0),
    warn: hex(0xf0b445, 1.0),
    success: hex(0x5fd39a, 1.0),
    edge: hex(0xffffff, 0.28),
    edge_closed: hex(0xffffff, 0.14),
    edge_dim: hex(0xffffff, 0.07),
    grid_dot: hex(0xffffff, 0.055),
    scrim: hex(0x000000, 0.5),
    shadow: hex(0x000000, 0.5),
};

pub static LIGHT: Theme = Theme {
    bg: hex(0xf2f2f4, 1.0),
    glass_alpha: 0.72,
    chrome: hex(0xffffff, 0.55),
    card: hex(0xffffff, 1.0),
    card_hover: hex(0xfafafb, 1.0),
    card_milestone: hex(0xececef, 1.0),
    card_milestone_hover: hex(0xe4e4e8, 1.0),
    overlay: hex(0xffffff, 0.97),
    control: hex(0x000000, 0.04),
    control_hover: hex(0x000000, 0.07),
    control_active: hex(0x000000, 0.12),
    field: hex(0xffffff, 0.9),
    hairline: hex(0x000000, 0.08),
    border_strong: hex(0x000000, 0.16),
    highlight: hex(0xffffff, 0.9),
    fg: hex(0x18181b, 1.0),
    fg_muted: hex(0x5c5c66, 1.0),
    fg_faint: hex(0x8a8a93, 1.0),
    emphasis: hex(0x111113, 1.0),
    on_emphasis: hex(0xffffff, 1.0),
    selection: hex(0x000000, 0.16),
    danger: hex(0xd92d2d, 1.0),
    warn: hex(0xb7791f, 1.0),
    success: hex(0x1a8f59, 1.0),
    edge: hex(0x000000, 0.3),
    edge_closed: hex(0x000000, 0.14),
    edge_dim: hex(0x000000, 0.07),
    grid_dot: hex(0x000000, 0.08),
    scrim: hex(0x000000, 0.25),
    shadow: hex(0x000000, 0.14),
};

#[cfg(test)]
mod tests {
    use super::*;

    fn linear(c: f32) -> f32 {
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }

    fn luminance(c: Rgba) -> f32 {
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    }

    /// `top` composited over the opaque `under`.
    fn over(top: Rgba, under: Rgba) -> Rgba {
        let mix = |t: f32, u: f32| t * top.a + u * (1. - top.a);
        Rgba { r: mix(top.r, under.r), g: mix(top.g, under.g), b: mix(top.b, under.b), a: 1. }
    }

    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn text_and_alarm_colors_meet_their_contrast_floors() {
        for (name, t) in [("dark", &DARK), ("light", &LIGHT)] {
            for (surface, bg) in [("bg", t.bg), ("card", t.card), ("overlay", over(t.overlay, t.bg))] {
                let floors = [
                    ("fg", t.fg, 7.),
                    ("fg_muted", t.fg_muted, 4.5),
                    ("fg_faint", t.fg_faint, 3.),
                    ("danger", t.danger, 3.),
                    ("warn", t.warn, 3.),
                    ("success", t.success, 3.),
                ];
                for (role, color, floor) in floors {
                    let ratio = contrast(color, bg);
                    assert!(ratio >= floor, "{name} {role} on {surface}: {ratio:.2} < {floor}");
                }
            }
        }
    }

    fn spread(c: Rgba) -> f32 {
        c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
    }

    #[test]
    fn surfaces_and_text_are_neutral_and_alarms_are_chromatic() {
        for (name, t) in [("dark", &DARK), ("light", &LIGHT)] {
            let neutral = [
                ("bg", t.bg),
                ("card", t.card),
                ("card_hover", t.card_hover),
                ("card_milestone", t.card_milestone),
                ("card_milestone_hover", t.card_milestone_hover),
                ("fg", t.fg),
                ("fg_muted", t.fg_muted),
                ("fg_faint", t.fg_faint),
                ("emphasis", t.emphasis),
                ("on_emphasis", t.on_emphasis),
            ];
            for (role, color) in neutral {
                assert!(spread(color) <= 0.04, "{name} {role} is tinted: {:.3}", spread(color));
            }
            for (role, color) in [("danger", t.danger), ("warn", t.warn), ("success", t.success)] {
                assert!(spread(color) >= 0.25, "{name} {role} is grey: {:.3}", spread(color));
            }
        }
    }
}

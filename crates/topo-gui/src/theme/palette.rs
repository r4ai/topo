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

/// Stands in for a signal role in a base theme; `mono` or `hued` always replaces it.
const UNSET: Rgba = hex(0x000000, 0.0);

/// `base` with every signal role mapped to a neutral of its own.
const fn mono(base: Theme) -> Theme {
    Theme {
        accent: base.emphasis,
        on_accent: base.on_emphasis,
        critical: base.emphasis,
        milestone: base.fg,
        status_done: base.fg_faint,
        ready: base.fg,
        priority_high: base.fg,
        md_heading: base.emphasis,
        md_link: base.fg,
        progress: base.fg_muted,
        progress_complete: base.fg,
        ..base
    }
}

/// `base` with the signal hues: `accent` for selection, focus and links, `warm` for the critical path and
/// milestones, `good` for done and ready.
const fn hued(base: Theme, accent: Rgba, on_accent: Rgba, warm: Rgba, good: Rgba, selection: Rgba) -> Theme {
    Theme {
        accent,
        on_accent,
        critical: warm,
        milestone: warm,
        status_done: good,
        ready: good,
        priority_high: warm,
        md_heading: accent,
        md_link: accent,
        progress: accent,
        progress_complete: good,
        selection,
        ..base
    }
}

const DARK_BASE: Theme = Theme {
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
    accent: UNSET,
    on_accent: UNSET,
    critical: UNSET,
    milestone: UNSET,
    status_done: UNSET,
    ready: UNSET,
    priority_high: UNSET,
    md_heading: UNSET,
    md_link: UNSET,
    progress: UNSET,
    progress_complete: UNSET,
};

const LIGHT_BASE: Theme = Theme {
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
    accent: UNSET,
    on_accent: UNSET,
    critical: UNSET,
    milestone: UNSET,
    status_done: UNSET,
    ready: UNSET,
    priority_high: UNSET,
    md_heading: UNSET,
    md_link: UNSET,
    progress: UNSET,
    progress_complete: UNSET,
};

pub static DARK_MONO: Theme = mono(DARK_BASE);
pub static LIGHT_MONO: Theme = mono(LIGHT_BASE);
pub static DARK: Theme = hued(
    DARK_BASE,
    hex(0x7aa7ff, 1.0),
    hex(0x0f0f11, 1.0),
    hex(0xf0b445, 1.0),
    hex(0x5fd39a, 1.0),
    hex(0x7aa7ff, 0.30),
);
pub static LIGHT: Theme = hued(
    LIGHT_BASE,
    hex(0x2f6fed, 1.0),
    hex(0xffffff, 1.0),
    hex(0xb7791f, 1.0),
    hex(0x1a8f59, 1.0),
    hex(0x2f6fed, 0.22),
);

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

    const ALL: [(&str, &Theme); 4] =
        [("dark", &DARK), ("light", &LIGHT), ("dark mono", &DARK_MONO), ("light mono", &LIGHT_MONO)];

    #[test]
    fn text_alarm_and_signal_colors_meet_their_contrast_floors() {
        for (name, t) in ALL {
            for (surface, bg) in [("bg", t.bg), ("card", t.card), ("overlay", over(t.overlay, t.bg))] {
                let floors = [
                    ("fg", t.fg, 7.),
                    ("fg_muted", t.fg_muted, 4.5),
                    ("fg_faint", t.fg_faint, 3.),
                    ("danger", t.danger, 3.),
                    ("warn", t.warn, 3.),
                    ("success", t.success, 3.),
                    ("accent", t.accent, 3.),
                    ("critical", t.critical, 3.),
                    ("milestone", t.milestone, 3.),
                    ("status_done", t.status_done, 3.),
                    ("ready", t.ready, 3.),
                    ("priority_high", t.priority_high, 3.),
                ];
                for (role, color, floor) in floors {
                    let ratio = contrast(color, bg);
                    assert!(ratio >= floor, "{name} {role} on {surface}: {ratio:.2} < {floor}");
                }
            }
            let ratio = contrast(t.on_accent, t.accent);
            assert!(ratio >= 4.5, "{name} on_accent on accent: {ratio:.2} < 4.5");
        }
    }

    fn spread(c: Rgba) -> f32 {
        c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
    }

    #[test]
    fn surfaces_and_text_are_neutral_and_alarms_are_chromatic() {
        for (name, t) in ALL {
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

    fn signals(t: &Theme) -> [(&'static str, Rgba); 11] {
        [
            ("accent", t.accent),
            ("on_accent", t.on_accent),
            ("critical", t.critical),
            ("milestone", t.milestone),
            ("status_done", t.status_done),
            ("ready", t.ready),
            ("priority_high", t.priority_high),
            ("md_heading", t.md_heading),
            ("md_link", t.md_link),
            ("progress", t.progress),
            ("progress_complete", t.progress_complete),
        ]
    }

    #[test]
    fn monotone_signals_are_achromatic_and_the_hued_ones_are_not() {
        for (name, t) in [("dark mono", &DARK_MONO), ("light mono", &LIGHT_MONO)] {
            for (role, color) in signals(t) {
                assert!(spread(color) <= 0.04, "{name} {role} is tinted: {:.3}", spread(color));
            }
        }
        for (name, t) in [("dark", &DARK), ("light", &LIGHT)] {
            for (role, color) in [
                ("accent", t.accent),
                ("critical", t.critical),
                ("milestone", t.milestone),
                ("status_done", t.status_done),
            ] {
                assert!(spread(color) >= 0.25, "{name} {role} is grey: {:.3}", spread(color));
            }
        }
    }

    #[test]
    fn a_theme_and_its_monotone_variant_differ_only_in_signals() {
        for (hued, mono) in [(&DARK, &DARK_MONO), (&LIGHT, &LIGHT_MONO)] {
            let strip = |t: &Theme| Theme { selection: UNSET, ..super::mono(*t) };
            assert_eq!(strip(hued), strip(mono));
        }
    }
}

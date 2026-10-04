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
    bg: hex(0x111216, 1.0),
    surface: hex(0x17181d, 1.0),
    card: hex(0x1e1f26, 1.0),
    card_hover: hex(0x25262f, 1.0),
    card_milestone: hex(0x221f1b, 1.0),
    card_milestone_hover: hex(0x2a2620, 1.0),
    raised: hex(0x2a2c36, 1.0),
    border: hex(0x2b2d37, 1.0),
    border_strong: hex(0x3b3e4a, 1.0),
    fg: hex(0xe8e9ee, 1.0),
    fg_muted: hex(0x9b9dab, 1.0),
    fg_faint: hex(0x626574, 1.0),
    accent: hex(0x6ea8fe, 1.0),
    success: hex(0x4cc38a, 1.0),
    warn: hex(0xf5b949, 1.0),
    danger: hex(0xf2555a, 1.0),
    grid_dot: hex(0x24262e, 1.0),
    link: hex(0x5ad1e6, 1.0),
    edge: hex(0x565a68, 1.0),
    scrim: hex(0x000000, 0.55),
    selection: hex(0x6ea8fe, 0x44 as f32 / 255.0),
    shadow: hex(0x000000, 0.45),
};

pub static LIGHT: Theme = Theme {
    bg: hex(0xf2f2f4, 1.0),
    surface: hex(0xf7f7f8, 1.0),
    card: hex(0xffffff, 1.0),
    card_hover: hex(0xfafafb, 1.0),
    card_milestone: hex(0xececef, 1.0),
    card_milestone_hover: hex(0xe4e4e8, 1.0),
    raised: hex(0xe6e6ea, 1.0),
    border: hex(0xe0e0e4, 1.0),
    border_strong: hex(0xc8c8ce, 1.0),
    fg: hex(0x18181b, 1.0),
    fg_muted: hex(0x5c5c66, 1.0),
    fg_faint: hex(0x8a8a93, 1.0),
    accent: hex(0x111113, 1.0),
    success: hex(0x1a8f59, 1.0),
    warn: hex(0xb7791f, 1.0),
    danger: hex(0xd92d2d, 1.0),
    grid_dot: hex(0xdcdce0, 1.0),
    link: hex(0x18181b, 1.0),
    edge: hex(0xb0b0b8, 1.0),
    scrim: hex(0x000000, 0.25),
    selection: hex(0x000000, 0.16),
    shadow: hex(0x000000, 0.14),
};

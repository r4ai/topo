//! Radius, space, type and control-size tokens, and the elevation shadows.
// Views adopt these as they migrate.
#![allow(dead_code)]

use gpui::{BoxShadow, point, px};

use super::current;

pub const R_XS: f32 = 4.;
pub const R_SM: f32 = 6.;
pub const R_MD: f32 = 8.;
pub const R_LG: f32 = 12.;
pub const R_XL: f32 = 16.;

pub const S1: f32 = 2.;
pub const S2: f32 = 4.;
pub const S3: f32 = 6.;
pub const S4: f32 = 8.;
pub const S5: f32 = 12.;
pub const S6: f32 = 16.;
pub const S7: f32 = 24.;
pub const S8: f32 = 32.;

pub const T_CAPTION: f32 = 10.;
pub const T_SMALL: f32 = 11.;
pub const T_BODY: f32 = 12.;
pub const T_BODY_LG: f32 = 13.;
pub const T_TITLE: f32 = 15.;
pub const T_HEADING: f32 = 18.;

pub const H_ICON: f32 = 22.;
pub const H_BUTTON: f32 = 26.;
pub const H_INPUT: f32 = 28.;
pub const H_TOOLBAR: f32 = 46.;

fn drop_shadow(y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color: current().shadow.into(),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// The 1px light edge along the top of glass.
fn top_highlight() -> BoxShadow {
    BoxShadow {
        color: current().highlight.into(),
        offset: point(px(0.), px(1.)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: true,
    }
}

/// A selected card.
pub fn e1() -> Vec<BoxShadow> {
    vec![drop_shadow(2., 8.)]
}

/// Pills, popovers and toasts.
pub fn e2() -> Vec<BoxShadow> {
    vec![drop_shadow(8., 24.), top_highlight()]
}

/// Dialogs and the palette.
pub fn e3() -> Vec<BoxShadow> {
    vec![drop_shadow(16., 48.), top_highlight()]
}

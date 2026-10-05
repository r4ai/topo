//! Motion: the app's durations, curves, and whether to animate at all.
//!
//! The policy is in `docs/gui/motion.md`. Views call the [`Motion`] extension to
//! animate an element as it enters; motion is skipped entirely, drawing the
//! final state at once, under reduced motion, in tests and for screenshots.

use std::cell::Cell;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Rgba, Styled, Window, point, px,
};

/// A dropdown or another small in-place reveal.
pub const FAST: Duration = Duration::from_millis(160);
/// A popover, a toast or the save bar.
pub const STANDARD: Duration = Duration::from_millis(240);
/// A dialog or the repository switcher.
pub const SLOW: Duration = Duration::from_millis(320);
/// A canvas camera move.
pub const CAMERA: Duration = Duration::from_millis(360);
/// Opacity settles before the surface finishes moving.
pub const FADE: Duration = Duration::from_millis(120);
/// How far a rising surface travels, in points.
pub const RISE: f32 = 6.;

thread_local! {
    /// Whether motion runs at all. Off under test, so frames stay deterministic.
    static ENABLED: Cell<bool> = const { Cell::new(!cfg!(test)) };
}

/// Turns motion on or off for this thread.
pub fn set_enabled(on: bool) {
    ENABLED.with(|cell| cell.set(on));
}

/// Whether motion runs.
pub fn enabled() -> bool {
    ENABLED.with(Cell::get) && !reduce_motion_requested()
}

/// Whether the environment asks for reduced motion.
pub fn reduce_motion_requested() -> bool {
    std::env::var_os("TOPO_REDUCE_MOTION").is_some_and(|value| truthy(&value)) || os_reduce_motion()
}

/// Whether an environment value asks for reduced motion.
fn truthy(value: &std::ffi::OsStr) -> bool {
    value == "1" || value == "true"
}

/// Decelerating easing: fast at first, gentle at rest.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// Position and velocity of a critically damped spring, in seconds.
/// Retargeting with the sampled velocity keeps camera motion continuous.
pub fn spring_state(from: f32, to: f32, velocity: f32, seconds: f32) -> (f32, f32) {
    const OMEGA: f32 = 30.;
    let displacement = from - to;
    let coefficient = velocity + OMEGA * displacement;
    let decay = (-OMEGA * seconds).exp();
    (to + (displacement + coefficient * seconds) * decay, (velocity - OMEGA * coefficient * seconds) * decay)
}

/// A non-bouncy spring arriving exactly at the destination in finite time.
pub fn settle(t: f32) -> f32 {
    let seconds = CAMERA.as_secs_f32();
    spring_state(0., 1., 0., t.clamp(0., 1.) * seconds).0 / spring_state(0., 1., 0., seconds).0
}

/// Adds the entering animations to any styled element.
pub trait Motion: IntoElement + Sized + 'static {
    /// Rebuilds `self` each frame with `animate`, or once at the end when motion is off.
    fn motion(
        self,
        id: impl Into<ElementId>,
        duration: Duration,
        easing: fn(f32) -> f32,
        animate: impl Fn(Self, f32) -> Self + 'static,
    ) -> AnyElement {
        match enabled() {
            true => self.with_animation(id, Animation::new(duration).with_easing(easing), animate).into_any_element(),
            false => animate(self, 1.).into_any_element(),
        }
    }

    /// Fades only a backdrop, so a child surface has its own timing and opacity.
    fn fade_background(self, id: impl Into<ElementId>, color: Rgba) -> AnyElement
    where
        Self: Styled,
    {
        self.motion(id, FADE, ease_out, move |element, t| element.bg(Rgba { a: color.a * t, ..color }))
    }

    /// Fades `self` in and rises it by [`RISE`] points.
    fn rise_in(self, id: impl Into<ElementId>, duration: Duration) -> AnyElement
    where
        Self: Styled,
    {
        Translated { element: Some(self), dy: RISE }.motion(
            id,
            duration,
            |t| t,
            move |mut surface, t| {
                let opacity = ease_out((t * duration.as_secs_f32() / FADE.as_secs_f32()).min(1.));
                surface.element = surface.element.map(|element| element.opacity(opacity));
                surface.dy = RISE * (1. - settle(t));
                surface
            },
        )
    }
}

impl<E: IntoElement + 'static> Motion for E {}

/// A visual offset applied after layout and anchoring, including to hitboxes.
/// The original dimensions, margins, sibling positions and scroll extent remain intact.
struct Translated<E> {
    element: Option<E>,
    dy: f32,
}

impl<E: IntoElement + 'static> IntoElement for Translated<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: IntoElement + 'static> Element for Translated<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut element = self.element.take().expect("motion is laid out once").into_any_element();
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        // An anchored menu can already be against the bottom edge. Reduce its
        // travel there instead of clipping content outside the viewport.
        let room = (f32::from(window.viewport_size().height - bounds.bottom())).max(0.);
        let dy = self.dy.min(room);
        window.with_element_offset(point(px(0.), px(dy)), |window| element.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

/// Whether macOS reports that the user asked for reduced motion.
#[cfg(all(target_os = "macos", not(test)))]
fn os_reduce_motion() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: NSWorkspace is a singleton and the getter has no side effects; the
    // selector is checked before it is sent, so an older system cannot crash.
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let responds: BOOL = msg_send![workspace, respondsToSelector: sel!(accessibilityDisplayShouldReduceMotion)];
        if responds == NO {
            return false;
        }
        let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce != NO
    }
}

#[cfg(any(not(target_os = "macos"), test))]
fn os_reduce_motion() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, anchored, deferred, div, prelude::*, size};
    use std::rc::Rc;

    struct MotionGuard(bool);

    impl MotionGuard {
        fn enabled() -> Self {
            let previous = ENABLED.with(Cell::get);
            set_enabled(true);
            Self(previous)
        }
    }

    impl Drop for MotionGuard {
        fn drop(&mut self) {
            set_enabled(self.0);
        }
    }

    struct Popup {
        y: f32,
        clicked: Rc<Cell<bool>>,
    }

    impl Render for Popup {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicked = self.clicked.clone();
            let menu = div()
                .id("menu")
                .debug_selector(|| "menu".into())
                .w(px(200.))
                .h(px(100.))
                .on_click(move |_, _, _| clicked.set(true))
                .rise_in("menu-motion", Duration::from_secs(10));
            div().size_full().child(deferred(
                anchored().position(point(px(100.), px(self.y))).snap_to_window_with_margin(px(8.)).child(menu),
            ))
        }
    }

    #[gpui::test]
    fn an_animated_popover_stays_inside_the_window_and_its_hitbox_moves_with_it(cx: &mut TestAppContext) {
        let _motion = MotionGuard::enabled();
        for y in [100., 498., 550.] {
            let clicked = Rc::new(Cell::new(false));
            let (entity, view) = cx.add_window_view(|_, _| Popup { y, clicked: clicked.clone() });
            view.simulate_resize(size(px(800.), px(600.)));
            view.run_until_parked();
            let entering = view.debug_bounds("menu").unwrap();
            assert!(entering.bottom() <= px(600.), "{entering:?}");
            assert_eq!(entering.size, size(px(200.), px(100.)));
            // This point is in the translated bottom strip, outside the resting
            // hitbox when there is room for the surface's full travel.
            view.simulate_click(point(entering.left() + px(50.), entering.bottom() - px(1.)), Modifiers::none());
            assert!(clicked.get());

            // Cancelling motion must draw the destination immediately, without
            // leaving any animated margin or changing anchoring measurements.
            set_enabled(false);
            entity.update(view, |_, cx| cx.notify());
            view.run_until_parked();
            let resting = view.debug_bounds("menu").unwrap();
            assert_eq!(resting.top(), px(if y + 100. > 600. { 492. } else { y }));
            assert_eq!(resting.size, entering.size);
            set_enabled(true);
        }
    }

    struct Column;

    impl Render for Column {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("surface")
                        .debug_selector(|| "surface".into())
                        .h(px(40.))
                        .mt(px(12.))
                        .rise_in("motion", Duration::from_secs(10)),
                )
                .child(div().debug_selector(|| "sibling".into()).h(px(40.)))
        }
    }

    #[gpui::test]
    fn surface_motion_preserves_existing_margins_and_sibling_layout(cx: &mut TestAppContext) {
        let _motion = MotionGuard::enabled();
        let (entity, view) = cx.add_window_view(|_, _| Column);
        view.run_until_parked();
        let sibling = view.debug_bounds("sibling").unwrap();
        let surface = view.debug_bounds("surface").unwrap();
        assert!(surface.top() > px(12.));
        assert_eq!(sibling.top(), px(52.));
        set_enabled(false);
        entity.update(view, |_, cx| cx.notify());
        view.run_until_parked();
        assert_eq!(view.debug_bounds("surface").unwrap().top(), px(12.));
        assert_eq!(view.debug_bounds("sibling").unwrap(), sibling);
    }

    #[test]
    fn the_surface_spring_is_monotonic_without_overshoot() {
        assert_eq!(settle(0.), 0.);
        assert_eq!(settle(1.), 1.);
        let mut previous = 0.;
        for step in 0..=100 {
            let position = settle(step as f32 / 100.);
            assert!((previous..=1.).contains(&position));
            previous = position;
        }
    }

    #[test]
    fn retargeting_a_spring_preserves_position_and_velocity() {
        let (position, velocity) = spring_state(0., 100., 0., 0.08);
        assert!(velocity > 0.);
        let (new_position, new_velocity) = spring_state(position, -50., velocity, 0.);
        assert!((new_position - position).abs() < 0.0001);
        assert!((new_velocity - velocity).abs() < 0.0001);
        let next = spring_state(position, -50., velocity, 0.0001).0;
        assert!(next > position, "reversing direction must decelerate instead of snapping");
    }

    #[test]
    fn ease_out_starts_at_zero_ends_at_one_and_decelerates() {
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        // Past the midpoint early, and monotonic.
        assert!(ease_out(0.5) > 0.5);
        assert!(ease_out(0.25) < ease_out(0.5));
        assert!((0. ..=1.).contains(&ease_out(0.3)));
    }

    #[test]
    fn motion_is_off_under_test_and_can_be_turned_on() {
        assert!(!enabled(), "tests must not animate");
        set_enabled(true);
        assert!(enabled());
        set_enabled(false);
    }

    #[test]
    fn the_environment_value_that_requests_reduced_motion_is_recognised() {
        use std::ffi::OsStr;
        assert!(truthy(OsStr::new("1")));
        assert!(truthy(OsStr::new("true")));
        assert!(!truthy(OsStr::new("0")));
        assert!(!truthy(OsStr::new("false")));
        assert!(!truthy(OsStr::new("")));
    }
}

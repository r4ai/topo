//! Motion: springs, the policy for whether they run, and the surface they move.
//!
//! Every animated value is a [`Spring`] in the SwiftUI response/damping model. It can be
//! retargeted at any moment and keeps its velocity, so an interrupted move never jumps.
//! Views drive values with the [`springs`] hook, keep a closing surface alive with
//! [`presence`] or [`Presence`], and draw it through [`surface`]. Springs rest at once in
//! tests and for screenshots; under reduced motion only crossfades run.

use std::cell::Cell;
use std::f32::consts::TAU;
use std::time::Instant;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, ParentElement as _, Pixels, Point, Rgba, Style, Styled, Window, div, point, px,
};

/// How a spring feels: `response` is the period of the undamped spring in seconds and
/// `damping` its damping ratio (1 never overshoots). A `spatial` preset moves or resizes
/// something, so it snaps under reduced motion.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Preset {
    pub response: f32,
    pub damping: f32,
    pub spatial: bool,
}

impl Preset {
    /// The undamped angular frequency, in radians per second.
    fn omega(self) -> f32 {
        TAU / self.response
    }
}

/// The app's springs.
pub mod preset {
    use super::Preset;

    /// Opacity and colour.
    pub const FADE: Preset = Preset { response: 0.14, damping: 1., spatial: false };
    /// Popovers, toasts, thumbs and press feedback.
    pub const SNAPPY: Preset = Preset { response: 0.23, damping: 0.86, spatial: true };
    /// Dialogs, the camera and reflow.
    pub const SMOOTH: Preset = Preset { response: 0.32, damping: 1., spatial: true };
    /// The completion check and the zoom's return from past its limit.
    pub const BOUNCY: Preset = Preset { response: 0.29, damping: 0.68, spatial: true };
    /// Pan inertia.
    pub const GLIDE: Preset = Preset { response: 0.90, damping: 1., spatial: true };
}

/// A value moving to a target on a damped spring. Sampling is a pure function of the time
/// since the last change, so the frame rate does not affect the path.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    from: f32,
    to: f32,
    velocity: f32,
    start: Instant,
    preset: Preset,
    /// How close to the target counts as arrived, in the value's own units.
    rest: f32,
}

impl Spring {
    /// A spring resting at `value`.
    pub fn settled(value: f32, preset: Preset, rest: f32, now: Instant) -> Self {
        Self { from: value, to: value, velocity: 0., start: now, preset, rest }
    }

    /// The value and its velocity per second: exactly the target and zero once settled.
    pub fn sample(&self, now: Instant) -> (f32, f32) {
        self.moving(now).unwrap_or((self.to, 0.))
    }

    /// Whether the spring has arrived. A settled spring stays settled until it is changed.
    pub fn is_settled(&self, now: Instant) -> bool {
        self.moving(now).is_none()
    }

    /// Moves towards `to`, continuing from the current value and velocity.
    pub fn retarget(&mut self, to: f32, now: Instant) {
        if to != self.to {
            self.rebase(now);
            self.to = to;
        }
    }

    /// Hands a gesture over: moves towards `to` from the current value at `velocity`.
    pub fn push(&mut self, to: f32, velocity: f32, now: Instant) {
        self.rebase(now);
        self.to = to;
        self.velocity = velocity;
    }

    /// Changes how the spring feels, continuing from the current value and velocity.
    pub fn set_preset(&mut self, preset: Preset, now: Instant) {
        if preset != self.preset {
            self.rebase(now);
            self.preset = preset;
        }
    }

    /// Where the spring is heading.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// Restarts the closed form from the current value and velocity.
    fn rebase(&mut self, now: Instant) {
        (self.from, self.velocity) = self.sample(now);
        self.start = now;
    }

    /// The value and velocity while the spring still moves. It has arrived once its energy,
    /// which only ever decays, fits within `rest`: neither the distance left nor the
    /// distance the remaining velocity could still cover exceeds it.
    fn moving(&self, now: Instant) -> Option<(f32, f32)> {
        let Preset { damping, .. } = self.preset;
        debug_assert!(damping > 0. && damping <= 1., "a spring is underdamped or critically damped");
        let omega = self.preset.omega();
        let seconds = now.saturating_duration_since(self.start).as_secs_f32();
        let (distance, velocity) = (self.from - self.to, self.velocity);
        let (distance, velocity) = if damping == 1. {
            let slope = velocity + omega * distance;
            let decay = (-omega * seconds).exp();
            ((distance + slope * seconds) * decay, (velocity - omega * slope * seconds) * decay)
        } else {
            let rate = damping * omega;
            let frequency = omega * (1. - damping * damping).sqrt();
            let swing = (velocity + rate * distance) / frequency;
            let (sin, cos) = (frequency * seconds).sin_cos();
            let decay = (-rate * seconds).exp();
            (
                (distance * cos + swing * sin) * decay,
                (velocity * cos - (rate * swing + frequency * distance) * sin) * decay,
            )
        };
        (distance.hypot(velocity / omega) > self.rest).then_some((self.to + distance, velocity))
    }
}

/// How far ahead of its current value a spring released at `velocity` should be targeted so
/// it starts without acceleration: a thrown object keeps its speed and then glides to rest.
pub fn projection(velocity: f32, preset: Preset) -> f32 {
    2. * preset.damping * velocity / preset.omega()
}

/// Linearly interpolates between two RGBA colors.
pub fn lerp(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    Rgba {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

thread_local! {
    /// Whether motion runs at all. Off under test, so frames stay deterministic.
    static ENABLED: Cell<bool> = const { Cell::new(!cfg!(test)) };
}

/// Turns motion on or off for this thread.
pub fn set_enabled(on: bool) {
    ENABLED.with(|cell| cell.set(on));
}

/// How much motion runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Every spring animates.
    Full,
    /// Reduced motion: movement and scaling snap, crossfades still run.
    Reduced,
    /// Every spring rests at its target at once: tests, screenshots and batch QA.
    Off,
}

impl Mode {
    fn of(enabled: bool, reduce: bool) -> Self {
        match (enabled, reduce) {
            (false, _) => Self::Off,
            (true, true) => Self::Reduced,
            (true, false) => Self::Full,
        }
    }

    /// Whether a spring with `preset` animates instead of snapping.
    fn animates(self, preset: Preset) -> bool {
        match self {
            Self::Full => true,
            Self::Reduced => !preset.spatial,
            Self::Off => false,
        }
    }
}

/// How much motion runs now, on this thread.
pub fn mode() -> Mode {
    Mode::of(ENABLED.with(Cell::get), reduce_motion_requested())
}

/// Whether the environment asks for reduced motion.
pub fn reduce_motion_requested() -> bool {
    std::env::var_os("TOPO_REDUCE_MOTION").is_some_and(|value| truthy(&value)) || os_reduce_motion()
}

/// Whether an environment value asks for reduced motion.
fn truthy(value: &std::ffi::OsStr) -> bool {
    value == "1" || value == "true"
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

/// How close a hooked spring comes before it rests. Hooked values are unit-interval
/// progress or points, and a five-hundredth of either is invisible.
const REST: f32 = 0.002;

/// Springs kept between frames that chase `targets`, returning their current values and
/// requesting another frame while any still moves.
///
/// The first call starts at `initial`, or at rest on `targets` without one. `preset: None`
/// snaps, and so does a preset that [`mode`] does not animate. A changed preset takes over
/// from the current value and velocity.
///
/// Call it while drawing: from `render`, `request_layout`, `prepaint` or `paint`, including
/// the closures of `canvas`. The springs are element state, so `id` is scoped by the ids of
/// the elements enclosing the call, and they are forgotten after a frame that does not ask
/// for them.
pub fn springs<const N: usize>(
    window: &mut Window,
    cx: &App,
    id: impl Into<ElementId>,
    initial: Option<[f32; N]>,
    targets: [f32; N],
    preset: Option<Preset>,
) -> [f32; N] {
    springs_within(window, cx, id, initial, targets, preset, REST)
}

/// [`springs`] for values in units of which `rest`, not a five-hundredth, is invisible.
pub fn springs_within<const N: usize>(
    window: &mut Window,
    cx: &App,
    id: impl Into<ElementId>,
    initial: Option<[f32; N]>,
    targets: [f32; N],
    preset: Option<Preset>,
    rest: f32,
) -> [f32; N] {
    let now = cx.background_executor().now();
    let mode = mode();
    let preset = preset.filter(|preset| mode.animates(*preset));
    let springs = window.with_global_id(id.into(), |id, window| {
        window.with_element_state(id, |springs, _| {
            let springs = chase(springs, initial, targets, preset, rest, now);
            (springs, springs)
        })
    });
    if springs.iter().any(|spring| !spring.is_settled(now)) {
        window.request_animation_frame();
    }
    springs.map(|spring| spring.sample(now).0)
}

/// One frame of [`springs`]: last frame's springs, retargeted.
fn chase<const N: usize>(
    springs: Option<[Spring; N]>,
    initial: Option<[f32; N]>,
    targets: [f32; N],
    preset: Option<Preset>,
    rest: f32,
    now: Instant,
) -> [Spring; N] {
    let Some(preset) = preset else {
        // A resting spring never reads its preset; the next animated frame sets it.
        return targets.map(|target| Spring::settled(target, preset::FADE, rest, now));
    };
    let mut springs =
        springs.unwrap_or_else(|| initial.unwrap_or(targets).map(|value| Spring::settled(value, preset, rest, now)));
    for (spring, target) in springs.iter_mut().zip(targets) {
        spring.set_preset(preset, now);
        spring.retarget(target, now);
    }
    springs
}

/// Whether to draw a surface that opens and closes, and how far in it is: `[opacity,
/// travel]`, both 0 when closed and 1 when open. Opacity follows [`preset::FADE`] and travel
/// follows `preset`, which may overshoot. `None` once the surface is closed and has faded
/// out. Call it every frame, open or not, from where [`springs`] may be called.
pub fn presence(
    window: &mut Window,
    cx: &App,
    id: impl Into<ElementId>,
    open: bool,
    preset: Preset,
) -> Option<[f32; 2]> {
    let target = [if open { 1. } else { 0. }];
    window.with_id(id, |window| {
        let [opacity] = springs(window, cx, "opacity", Some([0.]), target, Some(preset::FADE));
        let [travel] = springs(window, cx, "travel", Some([0.]), target, Some(preset));
        (open || opacity > 0.).then_some([opacity, travel])
    })
}

/// A payload that outlives its dismissal for as long as its surface takes to animate out.
pub struct Presence<T>(
    /// The item and whether it is still open.
    Option<(T, bool)>,
);

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> Presence<T> {
    /// Opens with `item`, replacing any earlier one without replaying the entrance.
    pub fn show(&mut self, item: T) {
        self.0 = Some((item, true));
    }

    /// Closes. The item stays drawable until its surface has faded out.
    pub fn hide(&mut self) {
        if let Some((_, open)) = &mut self.0 {
            *open = false;
        }
    }

    /// The open item: what the app acts on. `None` as soon as it is hidden.
    pub fn get(&self) -> Option<&T> {
        self.0.as_ref().filter(|(_, open)| *open).map(|(item, _)| item)
    }

    /// The item to draw, including while it animates out.
    pub fn shown(&self) -> Option<&T> {
        self.0.as_ref().map(|(item, _)| item)
    }

    /// [`presence`] for this item, which is dropped once it returns `None`. Call it every
    /// frame in which [`Self::shown`] is drawn.
    pub fn progress(
        &mut self,
        window: &mut Window,
        cx: &App,
        id: impl Into<ElementId>,
        preset: Preset,
    ) -> Option<[f32; 2]> {
        let progress = presence(window, cx, id, self.get().is_some(), preset);
        if progress.is_none() {
            self.0 = None;
        }
        progress
    }
}

/// The point of a surface that stays put while it scales.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pivot {
    Center,
    Bottom,
    TopLeft,
    TopRight,
}

impl Pivot {
    /// The pivot as fractions of the surface's width and height.
    fn unit(self) -> (f32, f32) {
        match self {
            Self::Center => (0.5, 0.5),
            Self::Bottom => (0.5, 1.),
            Self::TopLeft => (0., 0.),
            Self::TopRight => (1., 0.),
        }
    }
}

/// Draws `element` so that it can be moved, scaled, faded and made inert. The element is
/// laid out exactly as it would be alone: its size, margins, flex behaviour, sibling
/// positions and scroll extent are untouched.
///
/// Whatever the element hands to `deferred` or `anchored` is drawn later, outside the
/// surface, so wrap the content of a deferred element rather than the deferred element.
pub fn surface(element: impl IntoElement) -> Surface {
    Surface {
        element: Some(element.into_any_element()),
        offset: Point::default(),
        scale: 1.,
        pivot: Pivot::Center,
        opacity: 1.,
        inert: false,
    }
}

/// See [`surface`].
pub struct Surface {
    element: Option<AnyElement>,
    offset: Point<Pixels>,
    scale: f32,
    pivot: Pivot,
    opacity: f32,
    inert: bool,
}

impl Surface {
    /// Moves the element together with its hitboxes. Travel towards a window edge stops at
    /// that edge, so a surface resting against it is not pushed outside.
    pub fn offset(mut self, offset: Point<Pixels>) -> Self {
        self.offset = offset;
        self
    }

    /// Scales what the element paints about `pivot`, a point of its laid-out bounds.
    /// Hitboxes keep their size. `scale` must be positive.
    pub fn scale(mut self, scale: f32, pivot: Pivot) -> Self {
        self.scale = scale;
        self.pivot = pivot;
        self
    }

    /// Fades everything the element paints.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Paints the element as usual while none of its hitboxes takes pointer input: for a
    /// surface that is animating out.
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }
}

impl IntoElement for Surface {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Surface {
    /// What is prepainted and painted: the element, inside a fading wrapper when it fades.
    type RequestLayoutState = AnyElement;
    /// The offset applied.
    type PrepaintState = Point<Pixels>;

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
        let mut element = self.element.take().expect("a surface is laid out once");
        let layout = element.request_layout(window, cx);
        // gpui fades only what a `div` paints, and a `div` around the element would change
        // its layout. The fading `div` therefore gets a layout tree of its own, and its
        // one child draws the element where the real tree placed it.
        let drawn = match self.opacity < 1. {
            true => div().opacity(self.opacity).child(LaidOut(element)).into_any_element(),
            false => element,
        };
        (layout, drawn)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        drawn: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Point<Pixels> {
        if self.opacity < 1. {
            drawn.layout_as_root(AvailableSpace::min_size(), window, cx);
        }
        let viewport = window.viewport_size();
        let offset = point(
            travel(self.offset.x, bounds.left(), bounds.right(), viewport.width),
            travel(self.offset.y, bounds.top(), bounds.bottom(), viewport.height),
        );
        // A hitbox keeps the content mask it was inserted under and takes input only
        // inside it. Painting below uses the real mask.
        let mask = self.inert.then(|| ContentMask { bounds: Bounds::default() });
        window.with_content_mask(mask, |window| {
            window.with_element_offset(offset, |window| drawn.prepaint(window, cx));
        });
        offset
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        drawn: &mut AnyElement,
        offset: &mut Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (x, y) = self.pivot.unit();
        let pivot = bounds.origin + *offset + point(bounds.size.width * x, bounds.size.height * y);
        window.with_element_scale(self.scale, pivot, |window| drawn.paint(window, cx));
    }
}

/// `offset` along one axis, limited to the room between a surface spanning `near..far` and
/// the edges of a window `extent` long.
fn travel(offset: Pixels, near: Pixels, far: Pixels, extent: Pixels) -> Pixels {
    offset.max((-near).min(px(0.))).min((extent - far).max(px(0.)))
}

/// An element already laid out by a [`Surface`], drawn from inside its fading wrapper.
struct LaidOut(AnyElement);

impl IntoElement for LaidOut {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for LaidOut {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext, anchored, canvas, deferred, prelude::*,
        size,
    };
    use std::rc::Rc;
    use std::time::Duration;

    /// Turns motion on for a test and restores the previous setting afterwards.
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

    const PRESETS: [Preset; 5] = [preset::FADE, preset::SNAPPY, preset::SMOOTH, preset::BOUNCY, preset::GLIDE];

    fn ms(milliseconds: f32) -> Duration {
        Duration::from_secs_f32(milliseconds / 1000.)
    }

    /// Whether two values agree to within float rounding.
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1e-5 * a.abs().max(1.)
    }

    /// A spring released from rest at `from` towards `to`.
    fn released(from: f32, to: f32, preset: Preset, rest: f32, now: Instant) -> Spring {
        let mut spring = Spring::settled(from, preset, rest, now);
        spring.retarget(to, now);
        spring
    }

    #[test]
    fn a_spring_follows_the_same_path_at_any_frame_rate() {
        let start = Instant::now();
        for preset in PRESETS {
            let direct = released(0., 100., preset, 0., start);
            for hz in [60, 120] {
                // Restart the closed form every frame, as a retarget would.
                let mut stepped = direct;
                for frame in 1..=hz / 5 {
                    let now = start + Duration::from_secs(1) * frame / hz;
                    let velocity = stepped.sample(now).1;
                    stepped.push(100., velocity, now);
                }
                let (now, tolerance) = (start + ms(200.), 0.01);
                let ((value, velocity), (expected, expected_velocity)) = (stepped.sample(now), direct.sample(now));
                assert!((value - expected).abs() < tolerance, "{preset:?} at {hz} Hz: {value} != {expected}");
                assert!((velocity - expected_velocity).abs() < tolerance * preset.omega(), "{preset:?} at {hz} Hz");
            }
        }
    }

    #[test]
    fn retargeting_keeps_the_position_and_velocity_and_a_push_replaces_the_velocity() {
        let start = Instant::now();
        for preset in PRESETS {
            let mut spring = released(0., 100., preset, 0.25, start);
            let now = start + ms(80.);
            let (value, velocity) = spring.sample(now);
            assert!(value > 0. && value < 100. && velocity > 0., "{preset:?}");

            spring.retarget(-50., now);
            assert_eq!(spring.target(), -50.);
            let (retargeted, retargeted_velocity) = spring.sample(now);
            assert!(close(retargeted, value) && close(retargeted_velocity, velocity), "{preset:?}");
            assert!(spring.sample(now + ms(1.)).0 > value, "reversing must decelerate instead of snapping");

            spring.push(200., -300., now);
            let (pushed, pushed_velocity) = spring.sample(now);
            assert!(close(pushed, value) && pushed_velocity == -300., "{preset:?}");
            assert!(spring.sample(now + ms(1.)).0 < value, "a push starts at the gesture's velocity");
        }
    }

    #[test]
    fn only_the_underdamped_presets_overshoot_and_by_the_analytic_amount() {
        let start = Instant::now();
        let path = |preset| {
            let spring = released(0., 1., preset, 1e-6, start);
            (0..=8000).map(move |step| spring.sample(start + ms(step as f32 / 4.)).0)
        };
        for preset in [preset::SNAPPY, preset::BOUNCY] {
            let ratio = preset.damping / (1. - preset.damping * preset.damping).sqrt();
            let expected = (-std::f32::consts::PI * ratio).exp();
            let overshoot = path(preset).fold(0., f32::max) - 1.;
            assert!((overshoot - expected).abs() < 1e-4, "{preset:?}: {overshoot} != {expected}");
        }
        assert!(path(preset::SNAPPY).fold(0., f32::max) < 1.006, "snappy only hints at a bounce");
        assert!(path(preset::BOUNCY).fold(0., f32::max) > 1.05, "bouncy visibly bounces");
        for preset in [preset::FADE, preset::SMOOTH, preset::GLIDE] {
            let mut previous = 0.;
            for value in path(preset) {
                assert!((previous..=1.).contains(&value), "{preset:?}: {previous} then {value}");
                previous = value;
            }
            let arrived = released(0., 1., preset, 1e-6, start).sample(start + ms(10_000.));
            assert_eq!(arrived, (1., 0.), "{preset:?} arrives exactly");
        }
    }

    #[test]
    fn a_huge_move_settles_without_a_visible_final_snap() {
        let start = Instant::now();
        for preset in PRESETS {
            let spring = released(0., 100_000., preset, 0.25, start);
            let exact = released(0., 100_000., preset, 0., start);
            let settled = (1..600)
                .map(|frame| start + Duration::from_secs(1) * frame / 60)
                .find(|now| spring.is_settled(*now))
                .unwrap_or_else(|| panic!("{preset:?} never settled"));
            assert_eq!(spring.sample(settled), (100_000., 0.));
            let (value, velocity) = exact.sample(settled);
            assert!((value - 100_000.).abs() <= 0.25, "{preset:?} snapped from {value}");
            assert!(velocity.abs() <= 0.25 * preset.omega(), "{preset:?} stopped at {velocity}/s");
            assert!(spring.is_settled(settled + ms(1.)) && spring.is_settled(settled + ms(500.)), "{preset:?}");
        }
    }

    #[test]
    fn a_spring_pushed_to_its_projection_starts_without_acceleration() {
        let start = Instant::now();
        for preset in PRESETS {
            let velocity = 1000.;
            let mut spring = Spring::settled(40., preset, 0., start);
            spring.push(40. + projection(velocity, preset), velocity, start);
            let step = 1e-4;
            let acceleration = (spring.sample(start + Duration::from_secs_f32(step)).1 - velocity) / step;
            // Thrown at its own position instead, the spring would brake at 2ζωv.
            let braking = 2. * preset.damping * preset.omega() * velocity;
            assert!(acceleration.abs() < 0.01 * braking, "{preset:?}: {acceleration} against {braking}");
        }
    }

    #[test]
    fn motion_is_off_under_test_and_follows_the_reduced_motion_request_once_enabled() {
        assert_eq!(mode(), Mode::Off, "tests must not animate");
        {
            let _motion = MotionGuard::enabled();
            assert_eq!(mode(), Mode::of(true, reduce_motion_requested()));
        }
        assert_eq!(mode(), Mode::Off);
        assert_eq!(Mode::of(true, false), Mode::Full);
        assert_eq!(Mode::of(true, true), Mode::Reduced);
        assert_eq!(Mode::of(false, false), Mode::Off);
        assert_eq!(Mode::of(false, true), Mode::Off);
        for preset in PRESETS {
            assert!(Mode::Full.animates(preset));
            assert_eq!(Mode::Reduced.animates(preset), preset == preset::FADE, "only crossfades stay");
            assert!(!Mode::Off.animates(preset));
        }
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

    #[test]
    fn lerp_interpolates_each_channel() {
        use gpui::rgba;
        let from = rgba(0x00000000);
        let to = rgba(0x80A0C0FF);
        let mid = lerp(from, to, 0.5);
        assert!((mid.r - 0x40 as f32 / 255.).abs() < 1e-6);
        assert!((mid.g - 0x50 as f32 / 255.).abs() < 1e-6);
        assert!((mid.b - 0x60 as f32 / 255.).abs() < 1e-6);
        assert_eq!(mid.a, 0.5);
        assert_eq!(lerp(from, to, 0.), from);
        assert_eq!(lerp(from, to, 1.), to);
    }

    #[test]
    fn chasing_starts_at_the_initial_value_snaps_without_a_preset_and_rests_once_settled() {
        let start = Instant::now();
        let values = |springs: [Spring; 2], now| springs.map(|spring| spring.sample(now).0);
        let moving = |springs: [Spring; 2], now| springs.iter().any(|spring| !spring.is_settled(now));

        // What the hook does when the mode does not animate the preset.
        let snapped = chase(None, Some([0., 0.]), [1., 40.], None, REST, start);
        assert_eq!(values(snapped, start), [1., 40.]);
        assert!(!moving(snapped, start));

        let resting = chase(None, None, [1., 40.], Some(preset::SMOOTH), REST, start);
        assert_eq!(values(resting, start), [1., 40.]);
        assert!(!moving(resting, start));

        let entering = chase(None, Some([0., 0.]), [1., 40.], Some(preset::SMOOTH), REST, start);
        assert_eq!(values(entering, start), [0., 0.]);
        assert!(moving(entering, start));

        // The same targets on later frames leave the springs alone, so they come to rest.
        let later = start + ms(100.);
        let unchanged = chase(Some(entering), Some([0., 0.]), [1., 40.], Some(preset::SMOOTH), REST, later);
        assert_eq!(values(unchanged, later), values(entering, later));
        let end = start + ms(3000.);
        let arrived = chase(Some(unchanged), None, [1., 40.], Some(preset::SMOOTH), REST, end);
        assert_eq!(values(arrived, end), [1., 40.]);
        assert!(!moving(arrived, end), "a settled hook must stop asking for frames");

        // A new preset takes over where the old one was, and a lost preset snaps.
        let (value, velocity) = unchanged[1].sample(later);
        let swapped = chase(Some(unchanged), None, [1., 40.], Some(preset::BOUNCY), REST, later);
        let (swapped_value, swapped_velocity) = swapped[1].sample(later);
        assert!(close(swapped_value, value) && close(swapped_velocity, velocity));
        assert_ne!(swapped[1].sample(later + ms(50.)), unchanged[1].sample(later + ms(50.)));
        let reduced = chase(Some(swapped), None, [1., 40.], None, REST, later);
        assert_eq!(values(reduced, later), [1., 40.]);
    }

    fn redraw<V: 'static>(entity: &Entity<V>, cx: &mut VisualTestContext) {
        entity.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    struct Chaser {
        target: f32,
        seen: Rc<Cell<f32>>,
    }

    impl Render for Chaser {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let [value] = springs(window, cx, "chaser", Some([0.]), [self.target], Some(preset::SMOOTH));
            self.seen.set(value);
            div()
        }
    }

    #[gpui::test]
    fn the_hook_returns_its_targets_at_once_when_motion_is_off(cx: &mut TestAppContext) {
        let seen = Rc::new(Cell::new(f32::NAN));
        let (_, cx) = cx.add_window_view(|_, _| Chaser { target: 100., seen: seen.clone() });
        cx.run_until_parked();
        assert_eq!(seen.get(), 100.);
    }

    #[gpui::test]
    fn the_hook_follows_the_test_clock_and_retargets_mid_flight_without_a_jump(cx: &mut TestAppContext) {
        let _motion = MotionGuard::enabled();
        let seen = Rc::new(Cell::new(f32::NAN));
        let start = cx.executor().now();
        let (entity, cx) = cx.add_window_view(|_, _| Chaser { target: 100., seen: seen.clone() });
        cx.run_until_parked();
        assert_eq!(seen.get(), 0.);

        let mut expected = released(0., 100., preset::SMOOTH, REST, start);
        cx.executor().advance_clock(ms(100.));
        redraw(&entity, cx);
        let midway = seen.get();
        assert!(midway > 0. && midway < 100.);
        assert_eq!(midway, expected.sample(start + ms(100.)).0);

        entity.update(cx, |chaser, _| chaser.target = -50.);
        redraw(&entity, cx);
        assert!(close(seen.get(), midway), "a retarget continues from the current value");
        expected.retarget(-50., start + ms(100.));
        cx.executor().advance_clock(ms(4.));
        redraw(&entity, cx);
        assert!(seen.get() > midway, "and keeps its velocity");
        assert!(close(seen.get(), expected.sample(start + ms(104.)).0));

        cx.executor().advance_clock(ms(3000.));
        redraw(&entity, cx);
        assert_eq!(seen.get(), -50.);
    }

    struct Host {
        presence: Presence<Rc<()>>,
        seen: Rc<Cell<Option<[f32; 2]>>>,
    }

    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.seen.set(self.presence.progress(window, cx, "host", preset::SMOOTH));
            div()
        }
    }

    #[gpui::test]
    fn a_presence_keeps_its_item_until_the_exit_settles_and_reopens_from_where_it_was(cx: &mut TestAppContext) {
        let _motion = MotionGuard::enabled();
        let seen = Rc::new(Cell::new(None));
        let item = Rc::new(());
        let (entity, cx) = cx.add_window_view(|_, _| Host { presence: Presence::default(), seen: seen.clone() });
        cx.run_until_parked();
        assert_eq!(seen.get(), None, "nothing was shown yet");
        let step = |cx: &mut VisualTestContext, milliseconds| {
            cx.executor().advance_clock(ms(milliseconds));
            redraw(&entity, cx);
            seen.get()
        };

        entity.update(cx, |host, _| host.presence.show(item.clone()));
        assert_eq!(step(cx, 0.), Some([0., 0.]));
        let [opening, travel] = step(cx, 60.).unwrap();
        assert!(opening > 0. && opening < 1. && travel > 0. && travel < opening, "{opening} {travel}");

        entity.update(cx, |host, _| {
            host.presence.hide();
            assert!(host.presence.get().is_none(), "a hidden item is gone for the app");
            assert!(host.presence.shown().is_some(), "but is still drawn");
        });
        let [hidden, hidden_travel] = step(cx, 0.).unwrap();
        assert!(close(hidden, opening) && close(hidden_travel, travel), "the exit starts where the entrance was");
        let [closing, _] = step(cx, 80.).unwrap();
        assert!(closing > 0. && closing < opening);

        // Reopening during the exit carries on from the current opacity.
        entity.update(cx, |host, _| host.presence.show(item.clone()));
        assert!(close(step(cx, 0.).unwrap()[0], closing));
        assert_eq!(step(cx, 3000.), Some([1., 1.]));

        entity.update(cx, |host, _| host.presence.hide());
        assert!(step(cx, 80.).is_some());
        assert_eq!(Rc::strong_count(&item), 2, "the item lives while its surface leaves");
        assert_eq!(step(cx, 3000.), None);
        assert_eq!(Rc::strong_count(&item), 1, "and is dropped once it has faded out");
        entity.update(cx, |host, _| assert!(host.presence.shown().is_none()));
    }

    #[derive(Clone, Default)]
    struct Probe {
        clicked: Rc<Cell<bool>>,
        painted: Rc<Cell<bool>>,
        /// The content mask the probe paints under, which gpui expresses in unscaled coordinates.
        mask: Rc<Cell<Bounds<Pixels>>>,
    }

    impl Probe {
        /// A 200 by 100 clickable box.
        fn element(&self) -> impl IntoElement + use<> {
            let (clicked, painted, mask) = (self.clicked.clone(), self.painted.clone(), self.mask.clone());
            div()
                .id("probe")
                .debug_selector(|| "probe".into())
                .w(px(200.))
                .h(px(100.))
                .on_click(move |_, _, _| clicked.set(true))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, _| {
                            painted.set(true);
                            mask.set(window.content_mask().bounds);
                        },
                    )
                    .size_full(),
                )
        }

        fn click(&self, cx: &mut VisualTestContext, x: f32, y: f32) -> bool {
            cx.simulate_click(point(px(x), px(y)), Modifiers::none());
            self.clicked.replace(false)
        }
    }

    struct Stage {
        probe: Probe,
        build: fn(Surface) -> Surface,
    }

    impl Render for Stage {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().pt(px(50.)).pl(px(100.)).child((self.build)(surface(self.probe.element())))
        }
    }

    fn stage(cx: &mut TestAppContext, build: fn(Surface) -> Surface) -> (Probe, &mut VisualTestContext) {
        let probe = Probe::default();
        let (view, cx) = cx.add_window_view(|_, _| Stage { probe: probe.clone(), build });
        view.update(cx, |_, cx| cx.notify());
        cx.simulate_resize(size(px(800.), px(600.)));
        cx.run_until_parked();
        (probe, cx)
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[gpui::test]
    fn a_plain_surface_draws_its_element_in_place(cx: &mut TestAppContext) {
        let (probe, cx) = stage(cx, |surface| surface);
        assert_eq!(cx.debug_bounds("probe").unwrap(), rect(100., 50., 200., 100.));
        assert_eq!(probe.mask.get(), rect(0., 0., 800., 600.));
        assert!(probe.painted.get());
        assert!(probe.click(cx, 200., 100.));
    }

    #[gpui::test]
    fn an_inert_surface_paints_but_takes_no_clicks(cx: &mut TestAppContext) {
        for build in [|surface: Surface| surface.inert(true), |surface: Surface| surface.inert(true).opacity(0.5)] {
            let (probe, cx) = stage(cx, build);
            assert!(probe.painted.get());
            assert_eq!(probe.mask.get(), rect(0., 0., 800., 600.), "painting is not clipped");
            assert!(!probe.click(cx, 200., 100.));
        }
    }

    #[gpui::test]
    fn an_offset_moves_the_hitbox_and_a_scale_does_not(cx: &mut TestAppContext) {
        let (probe, cx) = stage(cx, |surface| surface.offset(point(px(30.), px(20.))));
        assert_eq!(cx.debug_bounds("probe").unwrap(), rect(130., 70., 200., 100.));
        assert!(probe.click(cx, 329., 169.), "the moved corner takes the click");
        assert!(!probe.click(cx, 110., 60.), "the vacated corner does not");

        let (probe, cx) = stage(cx, |surface| surface.scale(0.5, Pivot::TopRight));
        assert_eq!(cx.debug_bounds("probe").unwrap(), rect(100., 50., 200., 100.));
        assert!(probe.click(cx, 105., 145.), "the hitbox keeps its size while the picture shrinks");
        // Scaling by a half about (300, 50) maps the window (0, 0)..(800, 600) from this rectangle.
        assert_eq!(probe.mask.get(), rect(-300., -50., 1600., 1200.));

        let (probe, cx) =
            stage(cx, |surface| surface.offset(point(px(30.), px(20.))).scale(0.5, Pivot::Bottom).opacity(0.5));
        assert_eq!(cx.debug_bounds("probe").unwrap(), rect(130., 70., 200., 100.));
        assert!(probe.click(cx, 329., 169.));
        assert_eq!(probe.mask.get(), rect(-230., -170., 1600., 1200.), "the pivot moves with the offset");
    }

    struct Column {
        animated: bool,
    }

    impl Render for Column {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let item = div().debug_selector(|| "item".into()).flex_1().mt(px(12.)).mx(px(20.));
            let item = match self.animated {
                true => surface(item).offset(point(px(0.), px(6.))).scale(0.9, Pivot::Center).opacity(0.5),
                false => surface(item),
            };
            div().size_full().flex().flex_col().child(item).child(div().debug_selector(|| "sibling".into()).h(px(40.)))
        }
    }

    #[gpui::test]
    fn a_surface_preserves_margins_flex_sizing_and_sibling_layout(cx: &mut TestAppContext) {
        let (entity, cx) = cx.add_window_view(|_, _| Column { animated: false });
        cx.simulate_resize(size(px(800.), px(600.)));
        cx.run_until_parked();
        let (item, sibling) = (cx.debug_bounds("item").unwrap(), cx.debug_bounds("sibling").unwrap());
        assert_eq!(item, rect(20., 12., 760., 548.));
        assert_eq!(sibling, rect(0., 560., 800., 40.));

        entity.update(cx, |column, _| column.animated = true);
        redraw(&entity, cx);
        assert_eq!(cx.debug_bounds("item").unwrap(), rect(20., 18., 760., 548.));
        assert_eq!(cx.debug_bounds("sibling").unwrap(), sibling);
    }

    struct Popup {
        y: f32,
        offset: f32,
        probe: Probe,
    }

    impl Render for Popup {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let menu = surface(self.probe.element()).offset(point(px(0.), px(self.offset))).opacity(0.5);
            div().size_full().child(deferred(
                anchored().position(point(px(100.), px(self.y))).snap_to_window_with_margin(px(8.)).child(menu),
            ))
        }
    }

    #[gpui::test]
    fn an_anchored_surface_stays_inside_the_window_and_its_hitbox_moves_with_it(cx: &mut TestAppContext) {
        for y in [100., 490., 550.] {
            let probe = Probe::default();
            let (entity, cx) = cx.add_window_view(|_, _| Popup { y, offset: 6., probe: probe.clone() });
            cx.simulate_resize(size(px(800.), px(600.)));
            cx.run_until_parked();
            let resting = if y + 100. > 600. { 492. } else { y };
            let entering = cx.debug_bounds("probe").unwrap();
            // Against the bottom edge the surface travels less instead of leaving the window.
            assert_eq!(entering, rect(100., (resting + 6.).min(500.), 200., 100.));
            assert!(probe.click(cx, 150., f32::from(entering.bottom()) - 1.));

            entity.update(cx, |popup, _| popup.offset = 0.);
            redraw(&entity, cx);
            assert_eq!(cx.debug_bounds("probe").unwrap(), rect(100., resting, 200., 100.));
        }
    }
}

//! Experiment, enabled by `TOPO_GLASS=1` on macOS 26+: back the window with
//! AppKit's `NSGlassEffectView` (Liquid Glass) instead of GPUI's blur view.

#[cfg(all(target_os = "macos", not(test)))]
use gpui::Window;

/// True when the user opted in and this macOS ships `NSGlassEffectView`.
#[cfg(all(target_os = "macos", not(test)))]
pub fn requested() -> bool {
    std::env::var_os("TOPO_GLASS").is_some_and(|value| value == "1")
        && objc::runtime::Class::get("NSGlassEffectView").is_some()
}

#[cfg(all(target_os = "macos", not(test)))]
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect([f64; 4]);

// SAFETY: matches CGRect's layout, four CGFloats (origin x/y, size w/h).
#[cfg(all(target_os = "macos", not(test)))]
unsafe impl objc::Encode for Rect {
    fn encode() -> objc::Encoding {
        // SAFETY: the string is the Objective-C type encoding of CGRect on 64-bit macOS.
        unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

/// Inserts the glass view below GPUI's rendering view, as GPUI does for its blur view.
#[cfg(all(target_os = "macos", not(test)))]
pub fn install(window: &Window) {
    use objc::runtime::{BOOL, Class, Object};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const WIDTH_AND_HEIGHT_SIZABLE: u64 = 2 | 16;
    const WINDOW_BELOW: i64 = -1;
    let Some(glass_class) = Class::get("NSGlassEffectView") else {
        return;
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let ns_view = handle.ns_view.as_ptr() as *mut Object;
    // SAFETY: `ns_view` is GPUI's live NSView and this runs on the main thread. Every
    // receiver is nil-checked, and optional setters are guarded by respondsToSelector.
    unsafe {
        let ns_window: *mut Object = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        let content_view: *mut Object = msg_send![ns_window, contentView];
        if content_view.is_null() {
            return;
        }
        let bounds: Rect = msg_send![content_view, bounds];
        let glass: *mut Object = msg_send![glass_class, alloc];
        let glass: *mut Object = msg_send![glass, initWithFrame: bounds];
        if glass.is_null() {
            return;
        }
        let _: () = msg_send![glass, setAutoresizingMask: WIDTH_AND_HEIGHT_SIZABLE];
        let responds: BOOL = msg_send![glass, respondsToSelector: sel!(setCornerRadius:)];
        if responds != objc::runtime::NO {
            let _: () = msg_send![glass, setCornerRadius: 0.0f64];
        }
        let _: () = msg_send![content_view, addSubview: glass positioned: WINDOW_BELOW relativeTo: std::ptr::null_mut::<Object>()];
        let _: () = msg_send![glass, release];
    }
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn requested() -> bool {
    false
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn install(_window: &gpui::Window) {}

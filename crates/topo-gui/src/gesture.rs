//! Trackpad gestures gpui does not report: pinch to zoom and two-finger
//! double-tap ("smart zoom"), read from AppKit with a local event monitor.

use futures::channel::mpsc::{UnboundedReceiver, unbounded};

pub enum Gesture {
    /// Relative zoom step: the new scale is `1 + magnification` times the old one.
    Pinch(f32),
    SmartZoom,
}

/// Starts forwarding the gestures of this app's windows. The monitor lives
/// as long as the app.
#[cfg(target_os = "macos")]
pub fn watch() -> UnboundedReceiver<Gesture> {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    const MAGNIFY: u64 = 30;
    const SMART_MAGNIFY: u64 = 32;
    let (tx, rx) = unbounded();
    let handler = ConcreteBlock::new(move |event: *mut Object| -> *mut Object {
        // SAFETY: AppKit passes a valid NSEvent to local monitors on the main thread.
        let kind: u64 = unsafe { msg_send![event, type] };
        let gesture = match kind {
            MAGNIFY => {
                let magnification: f64 = unsafe { msg_send![event, magnification] };
                Some(Gesture::Pinch(magnification as f32))
            }
            SMART_MAGNIFY => Some(Gesture::SmartZoom),
            _ => None,
        };
        if let Some(gesture) = gesture {
            let _ = tx.unbounded_send(gesture);
        }
        event
    })
    .copy();
    let mask: u64 = (1 << MAGNIFY) | (1 << SMART_MAGNIFY);
    // SAFETY: the block has the `NSEvent *(^)(NSEvent *)` signature AppKit expects.
    let _monitor: *mut Object =
        unsafe { msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask handler: &*handler] };
    std::mem::forget(handler);
    rx
}

#[cfg(not(target_os = "macos"))]
pub fn watch() -> UnboundedReceiver<Gesture> {
    unbounded().1
}

//! Trackpad gestures gpui does not report: pinch to zoom and two-finger
//! double-tap ("smart zoom"), read from AppKit with a local event monitor.

use futures::channel::mpsc::{UnboundedReceiver, unbounded};

#[cfg_attr(all(not(target_os = "macos"), not(test)), allow(dead_code))]
pub enum Gesture {
    /// Relative zoom step: the new scale is `1 + magnification` times the old one.
    Pinch(f32),
    SmartZoom,
}

/// Owns the AppKit monitor; switching workspaces must remove the old one.
pub struct Monitor {
    #[cfg(all(target_os = "macos", not(test)))]
    native: *mut objc::runtime::Object,
}

#[cfg(all(target_os = "macos", not(test)))]
impl Drop for Monitor {
    fn drop(&mut self) {
        use objc::{class, msg_send, sel, sel_impl};
        // SAFETY: GPUI drops this view on the AppKit main thread. `native`
        // is the registered token returned by addLocalMonitorForEventsMatchingMask.
        unsafe {
            let _: () = msg_send![class!(NSEvent), removeMonitor: self.native];
        }
    }
}

/// Starts forwarding the gestures of this app's windows. The monitor lives
/// as long as its workspace view. Tests have no AppKit event loop to monitor.
#[cfg(all(target_os = "macos", not(test)))]
pub fn watch() -> (UnboundedReceiver<Gesture>, Monitor) {
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
    let monitor: *mut Object =
        unsafe { msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask handler: &*handler] };
    (rx, Monitor { native: monitor })
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn watch() -> (UnboundedReceiver<Gesture>, Monitor) {
    (unbounded().1, Monitor {})
}

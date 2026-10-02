//! Application identity for native launches, including unbundled `cargo run`.

/// Set the Dock and application switcher icon after GPUI starts AppKit.
#[cfg(all(target_os = "macos", not(test)))]
pub fn set_app_icon() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let bytes = include_bytes!("../../../assets/branding/topo.icns");
    // SAFETY: GPUI invokes this on AppKit's main thread. NSData copies the
    // embedded bytes; NSImage decodes that data and NSApplication retains the
    // image. Release our owned image after handing it to the application.
    unsafe {
        let data: *mut Object = msg_send![class!(NSData), dataWithBytes: bytes.as_ptr() length: bytes.len()];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if !image.is_null() {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setApplicationIconImage: image];
            let _: () = msg_send![image, release];
        }
    }
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn set_app_icon() {}

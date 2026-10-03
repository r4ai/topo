//! Application identity for native launches, including unbundled `cargo run`.

/// Set the Dock and application switcher icon after GPUI starts AppKit.
#[cfg(all(target_os = "macos", not(test)))]
pub fn set_app_icon() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // The ICNS already includes the macOS tile and transparent outer margin;
    // AppKit does not add a rounded mask to custom application icon images.
    let bytes = include_bytes!("../../../assets/branding/topo.icns");
    // SAFETY: GPUI invokes this on AppKit's main thread. NSData copies the
    // embedded bytes; NSImage decodes that data and NSApplication retains the
    // image. Release our owned image after handing it to the application.
    unsafe {
        // Bundled releases use Assets.car so macOS can render Liquid Glass and
        // appearance variants. A custom NSImage would replace that system icon.
        let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
        let key: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c"CFBundleIconName".as_ptr()];
        let icon_name: *mut Object = msg_send![bundle, objectForInfoDictionaryKey: key];
        if !icon_name.is_null() {
            return;
        }
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

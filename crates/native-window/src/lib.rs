#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
/// Configures the AppKit window used by Dioxus for opaque native rendering.
pub fn make_opaque(window: &(impl raw_window_handle::HasWindowHandle + ?Sized)) {
    use objc2_app_kit::{NSColorSpace, NSView};
    use raw_window_handle::RawWindowHandle;

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    // WindowHandle guarantees that ns_view remains valid for the borrow.
    unsafe {
        let view = &*handle.ns_view.as_ptr().cast::<NSView>();
        let Some(window) = view.window() else {
            return;
        };
        window.setOpaque(true);
        let color_space = NSColorSpace::sRGBColorSpace();
        window.setColorSpace(Some(&color_space));
    }
}

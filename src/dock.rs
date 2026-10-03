//! The icon in the dock, and whether there is one at all.
//!
//! All macOS. On Linux the dock is whatever the desktop provides and
//! there is no portable way to write on it, so the stubs do nothing and
//! say so by doing nothing.
//!
//! What a person chose — a window, a menu bar item, or both — is in
//! [`crate::preferences`]; this is only the drawing of it. The two are
//! close because on macOS asking for a menu bar alone means asking not
//! to be in the dock, which is also asking to give up the badge.

/// Takes the application out of the dock, or puts it back.
///
/// A menu bar application with an icon in the dock is two ways to reach
/// one window, and the dock one cannot be dismissed. `Accessory` is the
/// policy for a program that lives in the menu bar; `Regular` is the
/// one for a program that lives in a window.
#[cfg(target_os = "macos")]
pub fn in_the_dock(wanted: bool) {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(mtm);
    let policy = match wanted {
        true => NSApplicationActivationPolicy::Regular,
        false => NSApplicationActivationPolicy::Accessory,
    };
    application.setActivationPolicy(policy);
}

#[cfg(not(target_os = "macos"))]
pub fn in_the_dock(_wanted: bool) {}

/// Writes a count on the dock icon, or clears it.
///
/// The number is what needs a person — the same count the menu bar
/// item colours itself by — because a badge that counted sessions
/// would read as trouble on a fleet that is perfectly well.
#[cfg(target_os = "macos")]
pub fn badge(waiting: usize) {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let tile = NSApplication::sharedApplication(mtm).dockTile();
    match waiting {
        0 => tile.setBadgeLabel(None),
        count => tile.setBadgeLabel(Some(&NSString::from_str(&count.to_string()))),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn badge(_waiting: usize) {}

//! Photographing a window, from inside the process that owns it.
//!
/// Asking the window server for this window, by its number. A process
/// may photograph its own windows without the screen-recording
/// permission a capture of the whole display would need, which is what
/// makes the screenshots in the design document possible at all.
///
/// By its number, and not by the rectangle it occupies, because a
/// rectangle catches whatever is standing in front of it: a notification
/// banner, a screen saver, the lock screen, a window brought forward
/// mid-run. A contact sheet is twenty of these in half a minute, so
/// something was in the way often enough to be the normal case.
#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use anyhow::{anyhow, Result};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    /// On screen, excluding the desktop's own furniture.
    const ON_SCREEN_ONLY: u32 = 1;
    const EXCLUDE_DESKTOP: u32 = 16;
    /// This one window and nothing else, wherever it sits and whatever
    /// is in front of it.
    const INCLUDING_WINDOW: u32 = 8;
    /// The window alone, without the shadow the system draws around it,
    /// so the picture is the size the window is.
    const IGNORE_FRAMING: u32 = 1;
    /// Eight bits a channel, R G B A in that order in memory.
    const ALPHA_PREMULTIPLIED_LAST: u32 = 1;
    const BYTE_ORDER_32_BIG: u32 = 4 << 12;
    /// What `CGRectNull` is: the rectangle that asks for no rectangle,
    /// which is how a capture says "the window decides its own bounds".
    const NO_RECT: CGRect = CGRect {
        origin: CGPoint {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        size: CGSize {
            width: 0.,
            height: 0.,
        },
    };
    /// `kCFNumberSInt64Type`, the only one these fields need.
    const AS_I64: i64 = 4;
    /// The layer an ordinary window sits on. A menu bar item is higher,
    /// and this process may own one.
    const ORDINARY: i64 = 0;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, kind: i64, out: *mut c_void) -> bool;
        fn CFRelease(value: *const c_void);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> *const c_void;
        static kCGWindowOwnerPID: *const c_void;
        static kCGWindowNumber: *const c_void;
        static kCGWindowLayer: *const c_void;
        fn CGWindowListCreateImage(
            rect: CGRect,
            option: u32,
            window: u32,
            image_option: u32,
        ) -> *mut c_void;
        fn CGImageGetWidth(image: *mut c_void) -> usize;
        fn CGImageGetHeight(image: *mut c_void) -> usize;
        fn CGImageRelease(image: *mut c_void);
        fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
        fn CGColorSpaceRelease(space: *mut c_void);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            space: *mut c_void,
            info: u32,
        ) -> *mut c_void;
        fn CGContextDrawImage(context: *mut c_void, rect: CGRect, image: *mut c_void);
        fn CGContextRelease(context: *mut c_void);
    }

    /// One field of a window's description, as a number.
    unsafe fn field(entry: *const c_void, key: *const c_void) -> Option<i64> {
        let value = CFDictionaryGetValue(entry, key);
        if value.is_null() {
            return None;
        }
        let mut held: i64 = 0;
        CFNumberGetValue(value, AS_I64, (&mut held as *mut i64).cast()).then_some(held)
    }

    /// This process's own window, by number.
    ///
    /// The list comes back front to back, so the first ordinary window
    /// this process owns is the one in front — and a menu bar item,
    /// which this process may also own, sits on a higher layer and is
    /// passed over.
    fn mine() -> Option<u32> {
        let ours = std::process::id() as i64;
        unsafe {
            let list = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0);
            if list.is_null() {
                return None;
            }
            let mut found = None;
            for index in 0..CFArrayGetCount(list) {
                let entry = CFArrayGetValueAtIndex(list, index);
                if entry.is_null() {
                    continue;
                }
                if field(entry, kCGWindowOwnerPID) != Some(ours) {
                    continue;
                }
                if field(entry, kCGWindowLayer) != Some(ORDINARY) {
                    continue;
                }
                if let Some(number) = field(entry, kCGWindowNumber) {
                    found = Some(number as u32);
                    break;
                }
            }
            CFRelease(list);
            found
        }
    }

    pub(crate) fn grab(x: f64, y: f64, width: f64, height: f64) -> Result<image::RgbaImage> {
        // By number when the window server will name it, and by the
        // rectangle when it will not — a picture with a banner across it
        // beats no picture, and the caller already knows the bounds.
        let (rect, option, window, framing) = match mine() {
            Some(number) => (NO_RECT, INCLUDING_WINDOW, number, IGNORE_FRAMING),
            None => (
                CGRect {
                    origin: CGPoint { x, y },
                    size: CGSize { width, height },
                },
                ON_SCREEN_ONLY | EXCLUDE_DESKTOP,
                0,
                0,
            ),
        };
        unsafe {
            let image = CGWindowListCreateImage(rect, option, window, framing);
            if image.is_null() {
                return Err(anyhow!("the window server handed back no image"));
            }
            let width = CGImageGetWidth(image);
            let height = CGImageGetHeight(image);
            let mut pixels = vec![0u8; width * height * 4];
            let space = CGColorSpaceCreateDeviceRGB();
            let context = CGBitmapContextCreate(
                pixels.as_mut_ptr().cast(),
                width,
                height,
                8,
                width * 4,
                space,
                ALPHA_PREMULTIPLIED_LAST | BYTE_ORDER_32_BIG,
            );
            if context.is_null() {
                CGColorSpaceRelease(space);
                CGImageRelease(image);
                return Err(anyhow!("no bitmap to draw the frame into"));
            }
            CGContextDrawImage(
                context,
                CGRect {
                    origin: CGPoint { x: 0., y: 0. },
                    size: CGSize {
                        width: width as f64,
                        height: height as f64,
                    },
                },
                image,
            );
            CGContextRelease(context);
            CGColorSpaceRelease(space);
            CGImageRelease(image);
            image::RgbaImage::from_raw(width as u32, height as u32, pixels)
                .ok_or_else(|| anyhow!("the frame did not fit its own dimensions"))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    use anyhow::{anyhow, Result};

    pub(crate) fn grab(_x: f64, _y: f64, _w: f64, _h: f64) -> Result<image::RgbaImage> {
        Err(anyhow!("photographing the window is macOS only"))
    }
}

pub(crate) use mac::grab;

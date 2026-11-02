//! Keep a user-started massage active while the window is hidden. No assertions
//! remain after the background supervisor exits. Explicit system sleep is honored.
#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{c_char, c_void};
    type Id = *mut c_void;
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(allocator: Id, text: *const c_char, encoding: u32) -> Id;
        fn CFRelease(object: Id);
    }
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Id;
        fn objc_msgSend();
        fn objc_retain(object: Id) -> Id;
        fn objc_release(object: Id);
        fn objc_autoreleasePoolPush() -> Id;
        fn objc_autoreleasePoolPop(pool: Id);
    }
    pub struct Activity {
        process: Id,
        token: Id,
    }
    impl Activity {
        pub fn begin() -> Self {
            // The SDK declares NSUInteger options and id arguments/results on both
            // supported 64-bit macOS ABIs. Retain the token before draining the pool.
            unsafe {
                let pool = objc_autoreleasePoolPush();
                let get: unsafe extern "C" fn(Id, Id) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let begin: unsafe extern "C" fn(Id, Id, u64, Id) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let process = get(
                    objc_getClass(c"NSProcessInfo".as_ptr()),
                    sel_registerName(c"processInfo".as_ptr()),
                );
                let reason = CFStringCreateWithCString(
                    std::ptr::null_mut(),
                    c"JetSeat massage session".as_ptr(),
                    0x0800_0100,
                );
                let token = begin(
                    process,
                    sel_registerName(c"beginActivityWithOptions:reason:".as_ptr()),
                    0x00ff_ffff,
                    reason,
                );
                if !token.is_null() {
                    objc_retain(token);
                }
                if !reason.is_null() {
                    CFRelease(reason);
                }
                objc_autoreleasePoolPop(pool);
                Self { process, token }
            }
        }
    }
    impl Drop for Activity {
        fn drop(&mut self) {
            if !self.token.is_null() {
                unsafe {
                    let pool = objc_autoreleasePoolPush();
                    let end: unsafe extern "C" fn(Id, Id, Id) =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    end(
                        self.process,
                        sel_registerName(c"endActivity:".as_ptr()),
                        self.token,
                    );
                    objc_release(self.token);
                    objc_autoreleasePoolPop(pool);
                }
            }
        }
    }
}
#[cfg(target_os = "windows")]
mod platform {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }
    pub struct Activity;
    impl Activity {
        pub fn begin() -> Self {
            unsafe {
                SetThreadExecutionState(0x8000_0001);
            }
            Self
        }
    }
    impl Drop for Activity {
        fn drop(&mut self) {
            unsafe {
                SetThreadExecutionState(0x8000_0000);
            }
        }
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub struct Activity;
    impl Activity {
        pub fn begin() -> Self {
            Self
        }
    }
}
pub use platform::Activity;

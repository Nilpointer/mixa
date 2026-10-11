//! Thin, safe-ish wrappers around alsa-lib (via `alsa-sys`).
//!
//! Everything in here is single-threaded except [`watch`], which only uses
//! its own private ctl handle to wake up the UI on hardware changes.

pub mod card;
pub mod config;
pub mod hctl;
pub mod mixer;
pub mod watch;

use std::ffi::{CStr, CString, c_char};

/// Converts a (possibly null) C string pointer into an owned `String`.
pub(crate) fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

pub(crate) fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// Human-readable alsa error message.
pub fn strerror(err: i32) -> String {
    cstr(unsafe { alsa_sys::snd_strerror(err) })
}

/// Polls the given descriptors with a zero timeout and reports whether any is ready.
pub(crate) fn poll_ready(fds: &mut [libc::pollfd]) -> bool {
    if fds.is_empty() {
        return false;
    }
    for f in fds.iter_mut() {
        f.revents = 0;
    }
    let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 0) };
    n > 0
}

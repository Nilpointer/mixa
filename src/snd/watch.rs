//! Background thread that wakes the UI whenever a control on the device changes,
//! so the app can idle at zero CPU instead of polling.

use super::cstring;
use alsa_sys as sys;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub struct Watcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct Ctl(*mut sys::snd_ctl_t);
unsafe impl Send for Ctl {}

impl Watcher {
    /// Starts watching `device`. `wake` is called (from the watcher thread)
    /// on every control event and when the card list may have changed.
    pub fn start(device: &str, wake: impl Fn() + Send + 'static) -> Option<Watcher> {
        let dev = cstring(device);
        let mut ctl: *mut sys::snd_ctl_t = ptr::null_mut();
        if unsafe { sys::snd_ctl_open(&mut ctl, dev.as_ptr(), sys::SND_CTL_NONBLOCK as i32) } < 0 {
            return None;
        }
        if unsafe { sys::snd_ctl_subscribe_events(ctl, 1) } < 0 {
            unsafe { sys::snd_ctl_close(ctl) };
            return None;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let ctl = Ctl(ctl);
        let thread = std::thread::Builder::new()
            .name("alsa-watch".into())
            .spawn(move || {
                let ctl = ctl;
                let n = unsafe { sys::snd_ctl_poll_descriptors_count(ctl.0) }.max(0) as usize;
                let mut fds = vec![libc::pollfd { fd: 0, events: 0, revents: 0 }; n];
                unsafe { sys::snd_ctl_poll_descriptors(ctl.0, fds.as_mut_ptr() as *mut _, n as u32) };
                let mut ev: *mut sys::snd_ctl_event_t = ptr::null_mut();
                unsafe { sys::snd_ctl_event_malloc(&mut ev) };
                while !stop2.load(Ordering::Relaxed) {
                    let r = unsafe { libc::poll(fds.as_mut_ptr(), n as libc::nfds_t, 250) };
                    if r <= 0 {
                        continue;
                    }
                    if fds.iter().any(|f| f.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0) {
                        wake();
                        break;
                    }
                    // Drain all queued events, then wake the UI once.
                    while unsafe { sys::snd_ctl_read(ctl.0, ev) } > 0 {}
                    wake();
                }
                unsafe {
                    sys::snd_ctl_event_free(ev);
                    sys::snd_ctl_close(ctl.0);
                }
            })
            .ok()?;
        Some(Watcher { stop, thread: Some(thread) })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

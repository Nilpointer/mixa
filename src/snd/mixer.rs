//! ALSA simple mixer interface (the "alsamixer" view of a device).

use super::{cstr, cstring, poll_ready, strerror};
use alsa_sys as sys;
use std::ffi::c_long;
use std::ptr;

const DB_GAIN_MUTE: c_long = -9_999_999;
/// Ranges up to this many dB are mapped linearly (same rule as alsamixer).
const MAX_LINEAR_DB_SCALE: c_long = 24 * 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Dir {
    Playback = 0,
    Capture = 1,
}

impl Dir {
    pub const ALL: [Dir; 2] = [Dir::Playback, Dir::Capture];
    pub fn label(self) -> &'static str {
        match self {
            Dir::Playback => "Playback",
            Dir::Capture => "Capture",
        }
    }
}

/// Per-direction capabilities of a simple element.
#[derive(Clone, Debug, Default)]
pub struct DirInfo {
    /// ALSA channel ids present in this direction.
    pub channels: Vec<i32>,
    /// 0 = no volume, 1 = joined volume, n = independent channels.
    pub volume_channels: usize,
    pub vol_min: c_long,
    pub vol_max: c_long,
    /// dB range in 1/100 dB; equal values mean "no dB information".
    pub db_min: c_long,
    pub db_max: c_long,
    pub switch_channels: usize,
    pub enum_channels: usize,
}

impl DirInfo {
    pub fn has_volume(&self) -> bool {
        self.volume_channels > 0 && self.vol_max > self.vol_min
    }
    pub fn has_db(&self) -> bool {
        self.db_max > self.db_min
    }
    pub fn has_switch(&self) -> bool {
        self.switch_channels > 0
    }
    pub fn has_enum(&self) -> bool {
        self.enum_channels > 0
    }
    pub fn db_min_f(&self) -> f64 {
        self.db_min as f64 / 100.0
    }
    pub fn db_max_f(&self) -> f64 {
        self.db_max as f64 / 100.0
    }
}

/// A simple mixer element. The raw handle stays valid as long as the
/// owning [`Mixer`] lives.
pub struct Elem {
    h: *mut sys::snd_mixer_elem_t,
    pub name: String,
    pub index: u32,
    pub dirs: [DirInfo; 2],
    pub enum_items: Vec<String>,
}

impl Elem {
    /// Stable key used for persisting per-element UI state.
    pub fn key(&self) -> String {
        format!("{},{}", self.name, self.index)
    }
    pub fn display_name(&self) -> String {
        if self.index > 0 {
            format!("{} {}", self.name, self.index)
        } else {
            self.name.clone()
        }
    }
    pub fn dir(&self, d: Dir) -> &DirInfo {
        &self.dirs[d as usize]
    }
    pub fn is_active(&self) -> bool {
        unsafe { sys::snd_mixer_selem_is_active(self.h) > 0 }
    }
    pub fn channel_name(ch: i32) -> &'static str {
        match ch {
            0 => "FL",
            1 => "FR",
            2 => "RL",
            3 => "RR",
            4 => "FC",
            5 => "LFE",
            6 => "SL",
            7 => "SR",
            8 => "RC",
            _ => "CH",
        }
    }
    pub fn channel_long_name(ch: i32) -> String {
        cstr(unsafe { sys::snd_mixer_selem_channel_name(ch) })
    }

    // ----- volume

    fn chan(&self, d: Dir, idx: usize) -> i32 {
        self.dirs[d as usize].channels.get(idx).copied().unwrap_or(0)
    }

    pub fn volume(&self, d: Dir, idx: usize) -> c_long {
        let mut v: c_long = 0;
        let ch = self.chan(d, idx);
        unsafe {
            match d {
                Dir::Playback => sys::snd_mixer_selem_get_playback_volume(self.h, ch, &mut v),
                Dir::Capture => sys::snd_mixer_selem_get_capture_volume(self.h, ch, &mut v),
            };
        }
        v
    }

    pub fn set_volume(&self, d: Dir, idx: Option<usize>, v: c_long) {
        unsafe {
            match (d, idx) {
                (Dir::Playback, None) => sys::snd_mixer_selem_set_playback_volume_all(self.h, v),
                (Dir::Capture, None) => sys::snd_mixer_selem_set_capture_volume_all(self.h, v),
                (Dir::Playback, Some(i)) => {
                    sys::snd_mixer_selem_set_playback_volume(self.h, self.chan(d, i), v)
                }
                (Dir::Capture, Some(i)) => {
                    sys::snd_mixer_selem_set_capture_volume(self.h, self.chan(d, i), v)
                }
            };
        }
    }

    /// Current level in dB (only meaningful if `has_db`).
    pub fn db(&self, d: Dir, idx: usize) -> Option<f64> {
        let mut v: c_long = 0;
        let ch = self.chan(d, idx);
        let err = unsafe {
            match d {
                Dir::Playback => sys::snd_mixer_selem_get_playback_dB(self.h, ch, &mut v),
                Dir::Capture => sys::snd_mixer_selem_get_capture_dB(self.h, ch, &mut v),
            }
        };
        (err >= 0 && self.dir(d).has_db()).then_some(v as f64 / 100.0)
    }

    fn set_db(&self, d: Dir, idx: Option<usize>, mb: c_long, round: i32) {
        unsafe {
            match (d, idx) {
                (Dir::Playback, None) => sys::snd_mixer_selem_set_playback_dB_all(self.h, mb, round),
                (Dir::Capture, None) => sys::snd_mixer_selem_set_capture_dB_all(self.h, mb, round),
                (Dir::Playback, Some(i)) => {
                    sys::snd_mixer_selem_set_playback_dB(self.h, self.chan(d, i), mb, round)
                }
                (Dir::Capture, Some(i)) => {
                    sys::snd_mixer_selem_set_capture_dB(self.h, self.chan(d, i), mb, round)
                }
            };
        }
    }

    fn uses_perceptual(&self, d: Dir, perceptual: bool) -> bool {
        perceptual && self.dir(d).has_db()
    }

    /// Slider position (0..1) for the given channel.
    ///
    /// With `perceptual`, the alsamixer volume mapping is used: positions are
    /// spread evenly over the audible dB range instead of the raw register range.
    pub fn position(&self, d: Dir, idx: usize, perceptual: bool) -> f32 {
        let di = self.dir(d);
        if !self.uses_perceptual(d, perceptual) {
            let span = (di.vol_max - di.vol_min).max(1) as f32;
            return ((self.volume(d, idx) - di.vol_min) as f32 / span).clamp(0.0, 1.0);
        }
        let db = self.db(d, idx).unwrap_or(di.db_min_f());
        self.position_for_db(d, db, perceptual)
    }

    /// Slider position (0..1) a given dB value maps to.
    pub fn position_for_db(&self, d: Dir, db: f64, perceptual: bool) -> f32 {
        let di = self.dir(d);
        let mb = (db * 100.0) as c_long;
        if !self.uses_perceptual(d, perceptual) {
            // Linear in raw volume: ask alsa which raw value matches.
            let mut raw: c_long = 0;
            let err = unsafe {
                match d {
                    Dir::Playback => sys::snd_mixer_selem_ask_playback_dB_vol(self.h, mb, 0, &mut raw),
                    Dir::Capture => sys::snd_mixer_selem_ask_capture_dB_vol(self.h, mb, 0, &mut raw),
                }
            };
            if err < 0 {
                return 0.0;
            }
            let span = (di.vol_max - di.vol_min).max(1) as f32;
            return ((raw - di.vol_min) as f32 / span).clamp(0.0, 1.0);
        }
        let (min, max) = (di.db_min, di.db_max);
        if max - min <= MAX_LINEAR_DB_SCALE {
            return ((mb - min) as f32 / (max - min) as f32).clamp(0.0, 1.0);
        }
        let mut n = 10f64.powf((mb - max) as f64 / 6000.0);
        if min != DB_GAIN_MUTE {
            let min_norm = 10f64.powf((min - max) as f64 / 6000.0);
            n = (n - min_norm) / (1.0 - min_norm);
        }
        n.clamp(0.0, 1.0) as f32
    }

    /// Sets the slider position (0..1). `round` (-1/0/+1) decides which way to
    /// round when the hardware has coarser steps — pass the movement direction
    /// so that small moves always change the value.
    pub fn set_position(&self, d: Dir, idx: Option<usize>, pos: f32, perceptual: bool, round: i32) {
        let di = self.dir(d).clone();
        let pos = pos.clamp(0.0, 1.0) as f64;
        if !self.uses_perceptual(d, perceptual) {
            let span = (di.vol_max - di.vol_min) as f64;
            let raw = match round {
                r if r > 0 => (pos * span).ceil(),
                r if r < 0 => (pos * span).floor(),
                _ => (pos * span).round(),
            } as c_long
                + di.vol_min;
            self.set_volume(d, idx, raw);
            return;
        }
        let (min, max) = (di.db_min, di.db_max);
        let lrint_dir = |x: f64| -> c_long {
            match round {
                r if r > 0 => x.ceil() as c_long,
                r if r < 0 => x.floor() as c_long,
                _ => x.round() as c_long,
            }
        };
        let mb = if max - min <= MAX_LINEAR_DB_SCALE {
            lrint_dir(pos * (max - min) as f64) + min
        } else {
            let mut v = pos;
            if min != DB_GAIN_MUTE {
                let min_norm = 10f64.powf((min - max) as f64 / 6000.0);
                v = v * (1.0 - min_norm) + min_norm;
            }
            if v <= 0.0 { min } else { lrint_dir(6000.0 * v.log10()) + max }
        };
        self.set_db(d, idx, mb, if round == 0 { -1 } else { round });
    }

    // ----- switches

    pub fn switch(&self, d: Dir, idx: usize) -> bool {
        let mut v: i32 = 0;
        let ch = self.chan(d, idx);
        unsafe {
            match d {
                Dir::Playback => sys::snd_mixer_selem_get_playback_switch(self.h, ch, &mut v),
                Dir::Capture => sys::snd_mixer_selem_get_capture_switch(self.h, ch, &mut v),
            };
        }
        v != 0
    }

    /// True if any switch channel in this direction is on.
    pub fn switch_any(&self, d: Dir) -> bool {
        (0..self.dir(d).switch_channels.max(1)).any(|i| self.switch(d, i))
    }

    pub fn set_switch(&self, d: Dir, idx: Option<usize>, on: bool) {
        let v = on as i32;
        unsafe {
            match (d, idx) {
                (Dir::Playback, None) => sys::snd_mixer_selem_set_playback_switch_all(self.h, v),
                (Dir::Capture, None) => sys::snd_mixer_selem_set_capture_switch_all(self.h, v),
                (Dir::Playback, Some(i)) => {
                    sys::snd_mixer_selem_set_playback_switch(self.h, self.chan(d, i), v)
                }
                (Dir::Capture, Some(i)) => {
                    sys::snd_mixer_selem_set_capture_switch(self.h, self.chan(d, i), v)
                }
            };
        }
    }

    // ----- enumerations

    pub fn enum_item(&self, ch: usize) -> usize {
        let mut v: u32 = 0;
        unsafe { sys::snd_mixer_selem_get_enum_item(self.h, ch as i32, &mut v) };
        v as usize
    }

    pub fn set_enum_item(&self, ch: Option<usize>, item: usize) {
        let all = self.dirs.iter().map(|d| d.enum_channels).max().unwrap_or(1).max(1);
        let chans: Vec<usize> = match ch {
            Some(c) => vec![c],
            None => (0..all).collect(),
        };
        for c in chans {
            unsafe { sys::snd_mixer_selem_set_enum_item(self.h, c as i32, item as u32) };
        }
    }

    fn load(h: *mut sys::snd_mixer_elem_t) -> Elem {
        let name = cstr(unsafe { sys::snd_mixer_selem_get_name(h) });
        let index = unsafe { sys::snd_mixer_selem_get_index(h) };
        let mut dirs = [DirInfo::default(), DirInfo::default()];
        for ch in 0..=sys::SND_MIXER_SCHN_LAST {
            if unsafe { sys::snd_mixer_selem_has_playback_channel(h, ch) } > 0 {
                dirs[0].channels.push(ch);
            }
            if unsafe { sys::snd_mixer_selem_has_capture_channel(h, ch) } > 0 {
                dirs[1].channels.push(ch);
            }
        }
        unsafe {
            for (i, di) in dirs.iter_mut().enumerate() {
                let n = di.channels.len();
                if n == 0 {
                    continue;
                }
                let pb = i == 0;
                let (has_vol, vol_joined, has_sw, sw_joined) = if pb {
                    (
                        sys::snd_mixer_selem_has_playback_volume(h) > 0,
                        sys::snd_mixer_selem_has_playback_volume_joined(h) > 0,
                        sys::snd_mixer_selem_has_playback_switch(h) > 0,
                        sys::snd_mixer_selem_has_playback_switch_joined(h) > 0,
                    )
                } else {
                    (
                        sys::snd_mixer_selem_has_capture_volume(h) > 0,
                        sys::snd_mixer_selem_has_capture_volume_joined(h) > 0,
                        sys::snd_mixer_selem_has_capture_switch(h) > 0,
                        sys::snd_mixer_selem_has_capture_switch_joined(h) > 0,
                    )
                };
                if has_vol {
                    di.volume_channels = if vol_joined { 1 } else { n };
                    let (mut lo, mut hi, mut dlo, mut dhi) = (0, 0, 0, 0);
                    let err = if pb {
                        sys::snd_mixer_selem_get_playback_volume_range(h, &mut lo, &mut hi);
                        sys::snd_mixer_selem_get_playback_dB_range(h, &mut dlo, &mut dhi)
                    } else {
                        sys::snd_mixer_selem_get_capture_volume_range(h, &mut lo, &mut hi);
                        sys::snd_mixer_selem_get_capture_dB_range(h, &mut dlo, &mut dhi)
                    };
                    di.vol_min = lo;
                    di.vol_max = hi;
                    if err >= 0 {
                        di.db_min = dlo;
                        di.db_max = dhi;
                    }
                }
                if has_sw {
                    di.switch_channels = if sw_joined { 1 } else { n };
                }
            }
        }

        // Elements with a "common" volume/switch report it for both directions;
        // show it only once, on the side its name suggests.
        let l = name.to_lowercase();
        let capture_ish = ["mic", "capture", "boost", "input", "adc", "line in"].iter().any(|s| l.contains(s));
        let drop = if capture_ish { 0 } else { 1 };
        unsafe {
            if sys::snd_mixer_selem_has_common_volume(h) > 0 && dirs[0].has_volume() && dirs[1].has_volume() {
                dirs[drop].volume_channels = 0;
            }
            if sys::snd_mixer_selem_has_common_switch(h) > 0 && dirs[0].has_switch() && dirs[1].has_switch() {
                dirs[drop].switch_channels = 0;
            }
        }

        let mut enum_items = Vec::new();
        if unsafe { sys::snd_mixer_selem_is_enumerated(h) } > 0 {
            let mut is_pb = unsafe { sys::snd_mixer_selem_is_enum_playback(h) } > 0;
            let mut is_cap = unsafe { sys::snd_mixer_selem_is_enum_capture(h) } > 0;
            if !is_pb && !is_cap {
                // Same heuristic as qastools: guess the direction from the name.
                if ["mic", "adc ", "capture", "input source"].iter().any(|s| l.contains(s)) {
                    is_cap = true;
                } else {
                    is_pb = true;
                }
            }
            // Count channels that carry an enum value.
            let mut n = 0;
            for ch in 0..=sys::SND_MIXER_SCHN_LAST {
                let mut v: u32 = 0;
                if unsafe { sys::snd_mixer_selem_get_enum_item(h, ch, &mut v) } < 0 {
                    break;
                }
                n += 1;
            }
            let n = n.max(1);
            if is_pb {
                dirs[0].enum_channels = n;
            }
            if is_cap {
                dirs[1].enum_channels = n;
            }
            let count = unsafe { sys::snd_mixer_selem_get_enum_items(h) }.max(0);
            let mut buf = [0 as std::ffi::c_char; 128];
            for i in 0..count as u32 {
                let r = unsafe {
                    sys::snd_mixer_selem_get_enum_item_name(h, i, buf.len() - 1, buf.as_mut_ptr())
                };
                enum_items.push(if r >= 0 { cstr(buf.as_ptr()) } else { format!("#{i}") });
            }
        }

        Elem { h, name, index, dirs, enum_items }
    }
}

/// An open simple mixer for one device.
pub struct Mixer {
    h: *mut sys::snd_mixer_t,
    pub elems: Vec<Elem>,
    fds: Vec<libc::pollfd>,
}

impl Mixer {
    pub fn open(device: &str) -> Result<Mixer, String> {
        let mut h: *mut sys::snd_mixer_t = ptr::null_mut();
        let dev = cstring(device);
        unsafe {
            let err = sys::snd_mixer_open(&mut h, 0);
            if err < 0 {
                return Err(format!("Could not open mixer: {}", strerror(err)));
            }
            let err = sys::snd_mixer_attach(h, dev.as_ptr());
            if err < 0 {
                sys::snd_mixer_close(h);
                return Err(format!("Could not attach to {device}: {}", strerror(err)));
            }
            let err = sys::snd_mixer_selem_register(h, ptr::null_mut(), ptr::null_mut());
            if err < 0 {
                sys::snd_mixer_close(h);
                return Err(format!("Could not register simple mixer: {}", strerror(err)));
            }
            let err = sys::snd_mixer_load(h);
            if err < 0 {
                sys::snd_mixer_close(h);
                return Err(format!("Could not load mixer of {device}: {}", strerror(err)));
            }
        }
        let mut elems = Vec::new();
        let mut e = unsafe { sys::snd_mixer_first_elem(h) };
        while !e.is_null() {
            elems.push(Elem::load(e));
            e = unsafe { sys::snd_mixer_elem_next(e) };
        }
        let n = unsafe { sys::snd_mixer_poll_descriptors_count(h) }.max(0) as usize;
        let mut fds = vec![libc::pollfd { fd: 0, events: 0, revents: 0 }; n];
        if n > 0 {
            unsafe {
                sys::snd_mixer_poll_descriptors(h, fds.as_mut_ptr() as *mut _, n as u32);
            }
        }
        Ok(Mixer { h, elems, fds })
    }

    /// Processes pending hardware events so getters return fresh values.
    /// Returns `Err` if the device vanished (e.g. USB unplug).
    pub fn poll(&mut self) -> Result<bool, String> {
        if !poll_ready(&mut self.fds) {
            return Ok(false);
        }
        let mut revents: u16 = 0;
        unsafe {
            sys::snd_mixer_poll_descriptors_revents(
                self.h,
                self.fds.as_mut_ptr() as *mut _,
                self.fds.len() as u32,
                &mut revents,
            );
        }
        if revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) as u16 != 0 {
            return Err("The sound device was disconnected".into());
        }
        let err = unsafe { sys::snd_mixer_handle_events(self.h) };
        if err < 0 {
            return Err(format!("Mixer error: {}", strerror(err)));
        }
        Ok(true)
    }
}

impl Drop for Mixer {
    fn drop(&mut self) {
        unsafe { sys::snd_mixer_close(self.h) };
    }
}

//! ALSA high level control interface: every raw control element of a card.

use super::{cstr, cstring, poll_ready, strerror};
use alsa_sys as sys;
use std::ffi::c_long;
use std::ptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElemType {
    None,
    Boolean,
    Integer,
    Enumerated,
    Bytes,
    Iec958,
    Integer64,
}

impl ElemType {
    fn from_raw(t: u32) -> Self {
        match t {
            sys::SND_CTL_ELEM_TYPE_BOOLEAN => ElemType::Boolean,
            sys::SND_CTL_ELEM_TYPE_INTEGER => ElemType::Integer,
            sys::SND_CTL_ELEM_TYPE_ENUMERATED => ElemType::Enumerated,
            sys::SND_CTL_ELEM_TYPE_BYTES => ElemType::Bytes,
            sys::SND_CTL_ELEM_TYPE_IEC958 => ElemType::Iec958,
            sys::SND_CTL_ELEM_TYPE_INTEGER64 => ElemType::Integer64,
            _ => ElemType::None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ElemType::None => "None",
            ElemType::Boolean => "Boolean",
            ElemType::Integer => "Integer",
            ElemType::Enumerated => "Enumerated",
            ElemType::Bytes => "Bytes",
            ElemType::Iec958 => "IEC958",
            ElemType::Integer64 => "Integer64",
        }
    }
}

pub const IFACE_NAMES: [&str; 7] = ["Card", "HW dependent", "Mixer", "PCM", "Raw MIDI", "Timer", "Sequencer"];

pub struct HElem {
    h: *mut sys::snd_hctl_elem_t,
    id: *mut sys::snd_ctl_elem_id_t,
    pub numid: u32,
    pub iface: usize,
    pub name: String,
    pub index: u32,
    pub device: u32,
    pub subdevice: u32,
    pub ty: ElemType,
    pub count: usize,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub items: Vec<String>,
    pub readable: bool,
    pub writable: bool,
    pub volatile: bool,
    pub inactive: bool,
    pub locked: bool,
    pub user: bool,
    /// dB range in dB when the element carries a TLV dB scale.
    pub db_range: Option<(f64, f64)>,
}

/// Raw element value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Ints(Vec<i64>),
    Bytes(Vec<u8>),
    None,
}

impl HElem {
    pub fn iface_name(&self) -> &'static str {
        IFACE_NAMES.get(self.iface).copied().unwrap_or("Unknown")
    }

    fn info(h: *mut sys::snd_hctl_elem_t, ctl: *mut sys::snd_ctl_t) -> Option<HElem> {
        unsafe {
            let mut info: *mut sys::snd_ctl_elem_info_t = ptr::null_mut();
            sys::snd_ctl_elem_info_malloc(&mut info);
            if sys::snd_hctl_elem_info(h, info) < 0 {
                sys::snd_ctl_elem_info_free(info);
                return None;
            }
            let mut id: *mut sys::snd_ctl_elem_id_t = ptr::null_mut();
            sys::snd_ctl_elem_id_malloc(&mut id);
            sys::snd_hctl_elem_get_id(h, id);

            let ty = ElemType::from_raw(sys::snd_ctl_elem_info_get_type(info));
            let (min, max, step) = match ty {
                ElemType::Integer => (
                    sys::snd_ctl_elem_info_get_min(info) as i64,
                    sys::snd_ctl_elem_info_get_max(info) as i64,
                    sys::snd_ctl_elem_info_get_step(info) as i64,
                ),
                ElemType::Integer64 => (
                    sys::snd_ctl_elem_info_get_min64(info),
                    sys::snd_ctl_elem_info_get_max64(info),
                    sys::snd_ctl_elem_info_get_step64(info),
                ),
                ElemType::Boolean => (0, 1, 1),
                _ => (0, 0, 0),
            };
            let mut items = Vec::new();
            if ty == ElemType::Enumerated {
                let n = sys::snd_ctl_elem_info_get_items(info);
                for i in 0..n {
                    sys::snd_ctl_elem_info_set_item(info, i);
                    if sys::snd_hctl_elem_info(h, info) >= 0 {
                        items.push(cstr(sys::snd_ctl_elem_info_get_item_name(info)));
                    } else {
                        items.push(format!("#{i}"));
                    }
                }
            }
            let db_range = if ty == ElemType::Integer && sys::snd_ctl_elem_info_is_tlv_readable(info) > 0 {
                let (mut lo, mut hi): (c_long, c_long) = (0, 0);
                let err = sys::snd_ctl_get_dB_range(ctl, id, &mut lo, &mut hi);
                (err >= 0 && hi > lo).then(|| (lo as f64 / 100.0, hi as f64 / 100.0))
            } else {
                None
            };
            let e = HElem {
                h,
                id,
                numid: sys::snd_ctl_elem_info_get_numid(info),
                iface: sys::snd_ctl_elem_info_get_interface(info) as usize,
                name: cstr(sys::snd_ctl_elem_info_get_name(info)),
                index: sys::snd_ctl_elem_info_get_index(info),
                device: sys::snd_ctl_elem_info_get_device(info),
                subdevice: sys::snd_ctl_elem_info_get_subdevice(info),
                ty,
                count: sys::snd_ctl_elem_info_get_count(info) as usize,
                min,
                max,
                step,
                items,
                readable: sys::snd_ctl_elem_info_is_readable(info) > 0,
                writable: sys::snd_ctl_elem_info_is_writable(info) > 0,
                volatile: sys::snd_ctl_elem_info_is_volatile(info) > 0,
                inactive: sys::snd_ctl_elem_info_is_inactive(info) > 0,
                locked: sys::snd_ctl_elem_info_is_locked(info) > 0,
                user: sys::snd_ctl_elem_info_is_user(info) > 0,
                db_range,
            };
            sys::snd_ctl_elem_info_free(info);
            Some(e)
        }
    }

    pub fn read(&self) -> Value {
        if !self.readable {
            return Value::None;
        }
        unsafe {
            let mut v: *mut sys::snd_ctl_elem_value_t = ptr::null_mut();
            sys::snd_ctl_elem_value_malloc(&mut v);
            let res = if sys::snd_hctl_elem_read(self.h, v) < 0 {
                Value::None
            } else {
                match self.ty {
                    ElemType::Boolean => Value::Ints(
                        (0..self.count)
                            .map(|i| sys::snd_ctl_elem_value_get_boolean(v, i as u32) as i64)
                            .collect(),
                    ),
                    ElemType::Integer => Value::Ints(
                        (0..self.count)
                            .map(|i| sys::snd_ctl_elem_value_get_integer(v, i as u32) as i64)
                            .collect(),
                    ),
                    ElemType::Integer64 => Value::Ints(
                        (0..self.count)
                            .map(|i| sys::snd_ctl_elem_value_get_integer64(v, i as u32))
                            .collect(),
                    ),
                    ElemType::Enumerated => Value::Ints(
                        (0..self.count)
                            .map(|i| sys::snd_ctl_elem_value_get_enumerated(v, i as u32) as i64)
                            .collect(),
                    ),
                    ElemType::Bytes => Value::Bytes(
                        (0..self.count.min(512))
                            .map(|i| sys::snd_ctl_elem_value_get_byte(v, i as u32))
                            .collect(),
                    ),
                    ElemType::Iec958 => {
                        let mut iec: sys::snd_aes_iec958_t = std::mem::zeroed();
                        sys::snd_ctl_elem_value_get_iec958(v, &mut iec);
                        Value::Bytes(iec.status.to_vec())
                    }
                    ElemType::None => Value::None,
                }
            };
            sys::snd_ctl_elem_value_free(v);
            res
        }
    }

    /// Writes `val` to one channel (or all when `idx` is `None`).
    pub fn write(&self, idx: Option<usize>, val: i64) -> Result<(), String> {
        if !self.writable {
            return Err("Element is read-only".into());
        }
        let cur = match self.read() {
            Value::Ints(v) => v,
            _ => vec![0; self.count],
        };
        unsafe {
            let mut v: *mut sys::snd_ctl_elem_value_t = ptr::null_mut();
            sys::snd_ctl_elem_value_malloc(&mut v);
            sys::snd_ctl_elem_value_set_id(v, self.id);
            for i in 0..self.count {
                let x = if idx.is_none() || idx == Some(i) { val } else { cur.get(i).copied().unwrap_or(0) };
                let i = i as u32;
                match self.ty {
                    ElemType::Boolean => sys::snd_ctl_elem_value_set_boolean(v, i, x as c_long),
                    ElemType::Integer => sys::snd_ctl_elem_value_set_integer(v, i, x as c_long),
                    ElemType::Integer64 => sys::snd_ctl_elem_value_set_integer64(v, i, x),
                    ElemType::Enumerated => sys::snd_ctl_elem_value_set_enumerated(v, i, x as u32),
                    _ => {}
                }
            }
            let err = sys::snd_hctl_elem_write(self.h, v);
            sys::snd_ctl_elem_value_free(v);
            if err < 0 { Err(strerror(err)) } else { Ok(()) }
        }
    }

    /// Converts a raw integer value to dB using the element's TLV data.
    pub fn to_db(&self, ctl: &HCtl, raw: i64) -> Option<f64> {
        self.db_range?;
        let mut db: c_long = 0;
        let err = unsafe { sys::snd_ctl_convert_to_dB(ctl.ctl, self.id, raw as c_long, &mut db) };
        (err >= 0).then_some(db as f64 / 100.0)
    }
}

impl Drop for HElem {
    fn drop(&mut self) {
        unsafe { sys::snd_ctl_elem_id_free(self.id) };
    }
}

pub struct HCtl {
    h: *mut sys::snd_hctl_t,
    ctl: *mut sys::snd_ctl_t,
    fds: Vec<libc::pollfd>,
    pub elems: Vec<HElem>,
}

impl HCtl {
    pub fn open(device: &str) -> Result<HCtl, String> {
        let dev = cstring(device);
        let mut h: *mut sys::snd_hctl_t = ptr::null_mut();
        unsafe {
            let err = sys::snd_hctl_open(&mut h, dev.as_ptr(), sys::SND_CTL_NONBLOCK as i32);
            if err < 0 {
                return Err(format!("Could not open {device}: {}", strerror(err)));
            }
            let err = sys::snd_hctl_load(h);
            if err < 0 {
                sys::snd_hctl_close(h);
                return Err(format!("Could not load controls of {device}: {}", strerror(err)));
            }
        }
        let ctl = unsafe { sys::snd_hctl_ctl(h) };
        let mut elems = Vec::new();
        let mut e = unsafe { sys::snd_hctl_first_elem(h) };
        while !e.is_null() {
            if let Some(el) = HElem::info(e, ctl) {
                elems.push(el);
            }
            e = unsafe { sys::snd_hctl_elem_next(e) };
        }
        let n = unsafe { sys::snd_hctl_poll_descriptors_count(h) }.max(0) as usize;
        let mut fds = vec![libc::pollfd { fd: 0, events: 0, revents: 0 }; n];
        if n > 0 {
            unsafe { sys::snd_hctl_poll_descriptors(h, fds.as_mut_ptr() as *mut _, n as u32) };
        }
        Ok(HCtl { h, ctl, fds, elems })
    }

    /// Processes pending events. Returns `Err` when the device went away.
    pub fn poll(&mut self) -> Result<bool, String> {
        if !poll_ready(&mut self.fds) {
            return Ok(false);
        }
        if self.fds.iter().any(|f| f.revents & (libc::POLLERR | libc::POLLNVAL | libc::POLLHUP) != 0) {
            return Err("The sound device was disconnected".into());
        }
        let err = unsafe { sys::snd_hctl_handle_events(self.h) };
        if err < 0 {
            return Err(strerror(err));
        }
        Ok(true)
    }
}

impl Drop for HCtl {
    fn drop(&mut self) {
        self.elems.clear();
        unsafe { sys::snd_hctl_close(self.h) };
    }
}

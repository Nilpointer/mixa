//! Snapshot of the global ALSA configuration tree.

use super::{cstr, strerror};
use alsa_sys as sys;
use std::ptr;

#[derive(Clone, Debug)]
pub enum CfgValue {
    Integer(i64),
    Real(f64),
    Str(String),
    Pointer,
    Compound(Vec<CfgNode>),
}

#[derive(Clone, Debug)]
pub struct CfgNode {
    pub id: String,
    pub value: CfgValue,
}

impl CfgNode {
    pub fn type_name(&self) -> &'static str {
        match self.value {
            CfgValue::Integer(_) => "integer",
            CfgValue::Real(_) => "real",
            CfgValue::Str(_) => "string",
            CfgValue::Pointer => "pointer",
            CfgValue::Compound(_) => "compound",
        }
    }

    pub fn value_text(&self) -> String {
        match &self.value {
            CfgValue::Integer(i) => i.to_string(),
            CfgValue::Real(r) => r.to_string(),
            CfgValue::Str(s) => format!("\"{s}\""),
            CfgValue::Pointer => "<pointer>".into(),
            CfgValue::Compound(c) => format!("{} entries", c.len()),
        }
    }

    /// True if this node or any descendant matches `needle` (lowercase).
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() || self.id.to_lowercase().contains(needle) {
            return true;
        }
        match &self.value {
            CfgValue::Compound(c) => c.iter().any(|n| n.matches(needle)),
            _ => self.value_text().to_lowercase().contains(needle),
        }
    }

    unsafe fn read(cfg: *mut sys::snd_config_t) -> CfgNode {
        unsafe {
            let mut idp: *const std::ffi::c_char = ptr::null();
            sys::snd_config_get_id(cfg, &mut idp);
            let id = cstr(idp);
            let value = match sys::snd_config_get_type(cfg) {
                sys::SND_CONFIG_TYPE_INTEGER => {
                    let mut v = 0;
                    sys::snd_config_get_integer(cfg, &mut v);
                    CfgValue::Integer(v)
                }
                sys::SND_CONFIG_TYPE_INTEGER64 => {
                    let mut v = 0;
                    sys::snd_config_get_integer64(cfg, &mut v);
                    CfgValue::Integer(v)
                }
                sys::SND_CONFIG_TYPE_REAL => {
                    let mut v = 0.0;
                    sys::snd_config_get_real(cfg, &mut v);
                    CfgValue::Real(v)
                }
                sys::SND_CONFIG_TYPE_STRING => {
                    let mut p: *const std::ffi::c_char = ptr::null();
                    sys::snd_config_get_string(cfg, &mut p);
                    CfgValue::Str(cstr(p))
                }
                sys::SND_CONFIG_TYPE_COMPOUND => {
                    let mut children = Vec::new();
                    let mut it = sys::snd_config_iterator_first(cfg);
                    let end = sys::snd_config_iterator_end(cfg);
                    while it != end {
                        children.push(CfgNode::read(sys::snd_config_iterator_entry(it)));
                        it = sys::snd_config_iterator_next(it);
                    }
                    CfgValue::Compound(children)
                }
                _ => CfgValue::Pointer,
            };
            CfgNode { id, value }
        }
    }
}

/// Loads (or reloads) the global configuration and returns a snapshot of it.
pub fn load() -> Result<CfgNode, String> {
    unsafe {
        let mut top: *mut sys::snd_config_t = ptr::null_mut();
        let err = sys::snd_config_update_ref(&mut top);
        if err < 0 || top.is_null() {
            return Err(format!("Could not load ALSA configuration: {}", strerror(err)));
        }
        let mut root = CfgNode::read(top);
        root.id = "ALSA configuration".into();
        sys::snd_config_unref(top);
        Ok(root)
    }
}

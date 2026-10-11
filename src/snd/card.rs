use super::{cstr, cstring};
use alsa_sys as sys;
use std::ptr;

/// Static information about a sound card (from the ctl card info).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardInfo {
    pub index: i32,
    pub id: String,
    pub driver: String,
    pub name: String,
    pub long_name: String,
    pub mixer_name: String,
    pub components: String,
}

impl CardInfo {
    /// The ALSA device string for this card's hardware ctl.
    pub fn device(&self) -> String {
        format!("hw:{}", self.index)
    }
}

/// Lists all sound cards present in the system.
pub fn list_cards() -> Vec<CardInfo> {
    let mut cards = Vec::new();
    let mut idx: i32 = -1;
    loop {
        if unsafe { sys::snd_card_next(&mut idx) } < 0 || idx < 0 {
            break;
        }
        if let Some(info) = card_info(&format!("hw:{idx}")) {
            cards.push(info);
        }
    }
    cards
}

/// Reads card information for any ctl device name (e.g. `hw:0`, `default`).
pub fn card_info(device: &str) -> Option<CardInfo> {
    let name = cstring(device);
    let mut ctl: *mut sys::snd_ctl_t = ptr::null_mut();
    if unsafe { sys::snd_ctl_open(&mut ctl, name.as_ptr(), sys::SND_CTL_NONBLOCK as i32) } < 0 {
        return None;
    }
    let mut info: *mut sys::snd_ctl_card_info_t = ptr::null_mut();
    unsafe { sys::snd_ctl_card_info_malloc(&mut info) };
    let res = if unsafe { sys::snd_ctl_card_info(ctl, info) } >= 0 {
        unsafe {
            Some(CardInfo {
                index: sys::snd_ctl_card_info_get_card(info),
                id: cstr(sys::snd_ctl_card_info_get_id(info)),
                driver: cstr(sys::snd_ctl_card_info_get_driver(info)),
                name: cstr(sys::snd_ctl_card_info_get_name(info)),
                long_name: cstr(sys::snd_ctl_card_info_get_longname(info)),
                mixer_name: cstr(sys::snd_ctl_card_info_get_mixername(info)),
                components: cstr(sys::snd_ctl_card_info_get_components(info)),
            })
        }
    } else {
        None
    };
    unsafe {
        sys::snd_ctl_card_info_free(info);
        sys::snd_ctl_close(ctl);
    }
    res
}

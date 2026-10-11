//! "Controls" page: every raw ALSA control element (QasHctl equivalent).

use super::mixer_view::{empty_state, fmt_db, small_header};
use super::theme::{Palette, font, font_bold, font_mono, font_semibold, lerp_color, with_alpha};
use super::widgets::{self, badge, card_bg};
use crate::snd::hctl::{ElemType, HCtl, HElem, IFACE_NAMES, Value};
use eframe::egui::{self, Align2, CornerRadius, Id, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};

#[derive(Default)]
pub struct State {
    pub selected: Option<u32>,
    pub search: String,
    /// Interface filter; None = all.
    pub iface: Option<usize>,
    pub split: bool,
    pub error: Option<String>,
}

fn summary(e: &HElem, v: &Value) -> String {
    match (e.ty, v) {
        (_, Value::None) => if e.readable { "—".into() } else { "write-only".into() },
        (ElemType::Boolean, Value::Ints(x)) => {
            let on = x.iter().filter(|&&b| b != 0).count();
            if on == x.len() { "On".into() } else if on == 0 { "Off".into() } else { format!("{on}/{} on", x.len()) }
        }
        (ElemType::Enumerated, Value::Ints(x)) => {
            x.first().and_then(|&i| e.items.get(i as usize)).cloned().unwrap_or_default()
        }
        (_, Value::Ints(x)) => {
            if x.windows(2).all(|w| w[0] == w[1]) {
                x.first().map(|v| v.to_string()).unwrap_or_default()
            } else {
                x.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" · ")
            }
        }
        (_, Value::Bytes(b)) => format!("{} bytes", b.len()),
    }
}

pub fn show(ui: &mut Ui, hctl: &HCtl, st: &mut State) {
    let p = Palette::get(ui.ctx());
    let needle = st.search.to_lowercase();
    let visible: Vec<&HElem> = hctl
        .elems
        .iter()
        .filter(|e| st.iface.is_none_or(|i| e.iface == i))
        .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle) || e.numid.to_string() == needle)
        .collect();
    if st.selected.is_none_or(|n| !hctl.elems.iter().any(|e| e.numid == n)) {
        st.selected = visible.first().map(|e| e.numid);
    }

    // interface filter chips
    let mut ifaces: Vec<usize> = hctl.elems.iter().map(|e| e.iface).collect();
    ifaces.sort();
    ifaces.dedup();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if chip(ui, &format!("All  {}", hctl.elems.len()), st.iface.is_none()).clicked() {
            st.iface = None;
        }
        for i in ifaces {
            let n = hctl.elems.iter().filter(|e| e.iface == i).count();
            let name = IFACE_NAMES.get(i).copied().unwrap_or("?");
            if chip(ui, &format!("{name}  {n}"), st.iface == Some(i)).clicked() {
                st.iface = if st.iface == Some(i) { None } else { Some(i) };
            }
        }
    });
    ui.add_space(10.0);

    let avail = ui.available_size();
    let list_w = (avail.x * 0.42).clamp(280.0, 460.0);
    ui.horizontal_top(|ui| {
        // ---- list
        ui.allocate_ui_with_layout(vec2(list_w, avail.y), egui::Layout::top_down(egui::Align::Min), |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(list_w, avail.y - 4.0), Sense::hover());
            card_bg(ui.painter(), rect, &p, 0.0, 16);
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(6.0)));
            if visible.is_empty() {
                empty_state(&mut cui, &p, "No matches", "Try another search term");
            }
            let row_h = 50.0;
            ScrollArea::vertical().id_salt("hctl-list").auto_shrink([false, false]).show_rows(
                &mut cui,
                row_h,
                visible.len(),
                |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for e in &visible[range] {
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                        let sel = st.selected == Some(e.numid);
                        let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
                        let t = ui.ctx().animate_bool_with_time(resp.id.with("s"), sel, 0.15);
                        let bg = r.shrink2(vec2(2.0, 2.0));
                        if t > 0.0 || hov > 0.0 {
                            ui.painter().rect_filled(
                                bg,
                                CornerRadius::same(10),
                                lerp_color(with_alpha(p.surface_hi, hov), with_alpha(p.accent_mid(), 0.16), t),
                            );
                        }
                        if t > 0.0 {
                            let bar = egui::Rect::from_min_size(pos2(bg.left() + 2.0, bg.center().y - 11.0 * t), vec2(3.0, 22.0 * t));
                            widgets::gradient_rrect(ui.painter(), bar, 1.5, p.accent.b, p.accent.a, true);
                        }
                        let name_c = if e.inactive { p.text_faint } else { p.text };
                        let v = e.read();
                        let val = summary(e, &v);
                        let val_g = ui.painter().layout_no_wrap(val, font_semibold(12.0), if sel { p.accent_mid() } else { p.text_dim });
                        let val_w = val_g.size().x.min(120.0);
                        let name_g = ui.painter().layout_no_wrap(e.name.clone(), font_semibold(13.0), name_c);
                        let name_clip = egui::Rect::from_min_max(bg.min, pos2(bg.right() - val_w - 22.0, bg.bottom()));
                        ui.painter().with_clip_rect(name_clip).galley(pos2(bg.left() + 14.0, bg.top() + 7.0), name_g, name_c);
                        ui.painter().with_clip_rect(egui::Rect::from_min_max(pos2(bg.right() - val_w - 14.0, bg.top()), bg.max)).galley(
                            pos2(bg.right() - val_w - 12.0, bg.center().y - val_g.size().y / 2.0),
                            val_g,
                            p.text_dim,
                        );
                        let meta = format!(
                            "#{} · {} · {}{}",
                            e.numid,
                            e.iface_name(),
                            e.ty.label(),
                            if e.count > 1 { format!(" ×{}", e.count) } else { String::new() }
                        );
                        ui.painter().text(pos2(bg.left() + 14.0, bg.bottom() - 11.0), Align2::LEFT_CENTER, meta, font(10.5), p.text_faint);
                        if resp.clicked() {
                            st.selected = Some(e.numid);
                        }
                    }
                },
            );
        });

        ui.add_space(14.0);

        // ---- detail
        ui.vertical(|ui| {
            let Some(e) = st.selected.and_then(|n| hctl.elems.iter().find(|e| e.numid == n)) else {
                return;
            };
            ScrollArea::vertical().id_salt("hctl-detail").auto_shrink([false, false]).show(ui, |ui| {
                detail(ui, hctl, e, st);
            });
        });
    });
}

fn chip(ui: &mut Ui, text: &str, active: bool) -> egui::Response {
    let p = Palette::get(ui.ctx());
    let g = ui.painter().layout_no_wrap(text.to_string(), font_semibold(11.5), p.text);
    let size = g.size() + vec2(22.0, 12.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, active, 0.14);
    let hov = resp.hovered();
    let bg = lerp_color(if hov { p.surface_hi } else { p.surface }, with_alpha(p.accent_mid(), 0.2), t);
    ui.painter().rect_filled(r, CornerRadius::same((size.y / 2.0) as u8), bg);
    ui.painter().rect_stroke(
        r,
        CornerRadius::same((size.y / 2.0) as u8),
        Stroke::new(1.0, lerp_color(p.border, p.accent_mid(), t)),
        egui::StrokeKind::Inside,
    );
    let c = lerp_color(p.text_dim, p.text, t.max(if hov { 0.6 } else { 0.0 }));
    ui.painter().galley(r.center() - g.size() / 2.0, g, c);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn detail(ui: &mut Ui, hctl: &HCtl, e: &HElem, st: &mut State) {
    let p = Palette::get(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(&e.name).font(font_bold(22.0)));
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let accent_bg = with_alpha(p.accent_mid(), 0.18);
                badge(ui, e.iface_name(), p.text, accent_bg);
                badge(ui, e.ty.label(), p.text, p.surface_hi);
                if e.count > 1 {
                    badge(ui, &format!("{} channels", e.count), p.text, p.surface_hi);
                }
                let flag = |ui: &mut Ui, on: bool, txt: &str, c| {
                    if on {
                        badge(ui, txt, c, with_alpha(c, 0.14));
                    }
                };
                flag(ui, e.readable, "readable", p.ok);
                flag(ui, e.writable, "writable", p.ok);
                flag(ui, e.volatile, "volatile", p.accent_mid());
                flag(ui, e.inactive, "inactive", p.danger);
                flag(ui, e.locked, "locked", p.danger);
                flag(ui, e.user, "user", p.accent_mid());
            });
            ui.add_space(14.0);

            egui::Grid::new("hctl-info").num_columns(4).spacing(vec2(18.0, 8.0)).show(ui, |ui| {
                let kv = |ui: &mut Ui, k: &str, v: String| {
                    ui.label(RichText::new(k).font(font(11.5)).color(p.text_faint));
                    ui.label(RichText::new(v).font(font_mono(12.5)));
                };
                kv(ui, "Num ID", e.numid.to_string());
                kv(ui, "Index", e.index.to_string());
                ui.end_row();
                kv(ui, "Device", e.device.to_string());
                kv(ui, "Subdevice", e.subdevice.to_string());
                ui.end_row();
                if matches!(e.ty, ElemType::Integer | ElemType::Integer64) {
                    kv(ui, "Range", format!("{} … {}", e.min, e.max));
                    kv(ui, "Step", e.step.max(1).to_string());
                    ui.end_row();
                }
                if let Some((lo, hi)) = e.db_range {
                    kv(ui, "dB range", format!("{} … {}", fmt_db(lo), fmt_db(hi)));
                    ui.end_row();
                }
            });
        });

    ui.add_space(14.0);
    small_header(ui, &p, "Value");
    let value = e.read();
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let enabled = e.writable && !e.locked;
            let write = |idx: Option<usize>, v: i64, st: &mut State| {
                if let Err(err) = e.write(idx, v) {
                    st.error = Some(format!("Could not write {}: {err}", e.name));
                }
            };
            match (&value, e.ty) {
                (Value::None, _) => {
                    ui.label(RichText::new("This element has no readable value.").color(p.text_dim));
                }
                (Value::Ints(vals), ElemType::Boolean) => {
                    for (i, v) in vals.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(ch_label(e, i)).font(font_semibold(13.0)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_enabled_ui(enabled, |ui| {
                                    if widgets::toggle(ui, *v != 0, None).clicked() {
                                        write(Some(i), (*v == 0) as i64, st);
                                    }
                                });
                            });
                        });
                    }
                }
                (Value::Ints(vals), ElemType::Integer | ElemType::Integer64) => {
                    if vals.len() > 1 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Link channels").font(font(12.5)).color(p.text_dim));
                            let mut linked = !st.split;
                            if widgets::toggle(ui, linked, None).clicked() {
                                linked = !linked;
                                st.split = !linked;
                            }
                        });
                        ui.add_space(6.0);
                    }
                    let span = (e.max - e.min).max(1) as f64;
                    let show: Vec<usize> = if st.split || vals.len() == 1 { (0..vals.len()).collect() } else { vec![0] };
                    for i in show {
                        let v = vals[i];
                        ui.horizontal(|ui| {
                            let lbl = if !st.split && vals.len() > 1 { "All".to_string() } else { ch_label(e, i) };
                            ui.add_sized(vec2(70.0, 26.0), egui::Label::new(RichText::new(lbl).font(font_semibold(12.5))));
                            let w = (ui.available_width() - 140.0).max(120.0);
                            let pos = ((v - e.min) as f64 / span) as f32;
                            let (nv, _) = widgets::hslider(ui, Id::new(("hs", e.numid, i)), w, pos, enabled);
                            let mut text = v.to_string();
                            if let Some(db) = e.to_db(hctl, v) {
                                text = format!("{v}  ·  {}", fmt_db(db));
                            }
                            ui.label(RichText::new(text).font(font_mono(12.0)).color(p.accent_mid()));
                            if let Some(nv) = nv {
                                let step = e.step.max(1);
                                let raw = e.min + (((nv as f64 * span) / step as f64).round() as i64) * step;
                                if raw != v {
                                    write(if st.split { Some(i) } else { None }, raw.clamp(e.min, e.max), st);
                                }
                            }
                        });
                    }
                }
                (Value::Ints(vals), ElemType::Enumerated) => {
                    for (i, v) in vals.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_sized(vec2(90.0, 28.0), egui::Label::new(RichText::new(ch_label(e, i)).font(font_semibold(12.5))));
                            let mut sel = *v as usize;
                            ui.add_enabled_ui(enabled, |ui| {
                                egui::ComboBox::from_id_salt(("hctl-enum", e.numid, i))
                                    .width(220.0)
                                    .selected_text(e.items.get(sel).cloned().unwrap_or_default())
                                    .show_ui(ui, |ui| {
                                        for (k, it) in e.items.iter().enumerate() {
                                            ui.selectable_value(&mut sel, k, it);
                                        }
                                    });
                            });
                            if sel != *v as usize {
                                write(Some(i), sel as i64, st);
                            }
                        });
                    }
                }
                (Value::Bytes(b), _) => {
                    let mut s = String::new();
                    for (i, chunk) in b.chunks(16).enumerate() {
                        s.push_str(&format!("{:04x}  ", i * 16));
                        for x in chunk {
                            s.push_str(&format!("{x:02x} "));
                        }
                        s.push('\n');
                    }
                    ui.label(RichText::new(s).font(font_mono(12.0)).color(p.text_dim));
                }
                _ => {}
            }
        });
}

fn ch_label(e: &HElem, i: usize) -> String {
    if e.count <= 1 {
        "Value".into()
    } else if e.count == 2 {
        ["Left", "Right"][i].into()
    } else {
        format!("Channel {}", i + 1)
    }
}

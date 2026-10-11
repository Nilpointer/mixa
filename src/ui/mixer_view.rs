//! The main mixer page: channel strips, switches and selections.

use super::settings::Settings;
use super::theme::{Palette, font, font_semibold, lerp_color, with_alpha};
use super::widgets::{self, Ticks, card_bg, icon, icon_button};
use crate::snd::mixer::{Dir, Elem, Mixer};
use eframe::egui::{self, Align2, Id, Rect, ScrollArea, Sense, Ui, pos2, vec2};

/// Unique key for per-element settings (hidden / split).
fn elem_key(dev: &str, e: &Elem, d: Dir) -> String {
    format!("{dev}|{}|{:?}", e.key(), d)
}

pub fn fmt_db(db: f64) -> String {
    if db <= -99.0 {
        "−∞ dB".into()
    } else if db.abs() < 0.05 {
        "0.0 dB".into()
    } else {
        format!("{}{:.1} dB", if db < 0.0 { "−" } else { "+" }, db.abs())
    }
}

fn value_text(e: &Elem, d: Dir, ch: usize, s: &Settings) -> String {
    if s.show_db
        && let Some(db) = e.db(d, ch) {
            let di = e.dir(d);
            if (db - di.db_min_f()).abs() < 0.01 && di.db_min <= -9_999_999 {
                return fmt_db(-1000.0);
            }
            return fmt_db(db);
        }
    format!("{:.0}%", e.position(d, ch, s.perceptual) * 100.0)
}

/// Scale marks for a fader of the given track length.
fn tick_marks(e: &Elem, d: Dir, s: &Settings, track_len: f32) -> Vec<(f32, String)> {
    let di = e.dir(d);
    let mut out: Vec<(f32, String)> = Vec::new();
    let min_gap = 20.0 / track_len.max(1.0);
    if di.has_db() {
        let candidates = [30.0, 20.0, 12.0, 6.0, 0.0, -3.0, -6.0, -10.0, -15.0, -20.0, -30.0, -40.0, -50.0, -60.0, -80.0];
        for db in candidates {
            if db > di.db_max_f() + 0.01 || db < di.db_min_f() - 0.01 {
                continue;
            }
            let pos = e.position_for_db(d, db, s.perceptual);
            if out.iter().all(|(p, _)| (p - pos).abs() >= min_gap) {
                let label = if db > 0.0 { format!("+{db:.0}") } else if db < 0.0 { format!("−{:.0}", -db) } else { "0".into() };
                out.push((pos, label));
            }
        }
    } else {
        for pct in [0, 25, 50, 75, 100] {
            out.push((pct as f32 / 100.0, pct.to_string()));
        }
    }
    out
}

pub struct MixerPage<'a> {
    pub mixer: &'a Mixer,
    pub settings: &'a mut Settings,
    pub dev_key: &'a str,
}

impl MixerPage<'_> {
    pub fn show(&mut self, ui: &mut Ui) {
        let p = Palette::get(ui.ctx());
        let dirs: Vec<Dir> = match self.settings.view {
            1 => vec![Dir::Playback],
            2 => vec![Dir::Capture],
            _ => Dir::ALL.to_vec(),
        };
        let show_hidden = self.settings.show_hidden;
        let visible = |s: &Settings, e: &Elem, d: Dir| show_hidden || !s.hidden.contains(&elem_key(self.dev_key, e, d));

        let rows: Vec<(Dir, Vec<&Elem>)> = dirs
            .iter()
            .map(|&d| {
                let v = self.mixer.elems.iter().filter(|e| e.dir(d).has_volume() && visible(self.settings, e, d)).collect();
                (d, v)
            })
            .filter(|(_, v): &(Dir, Vec<&Elem>)| !v.is_empty())
            .collect();

        let mut switches: Vec<(Dir, &Elem)> = Vec::new();
        let mut enums: Vec<(Dir, &Elem)> = Vec::new();
        for &d in &dirs {
            for e in &self.mixer.elems {
                if !visible(self.settings, e, d) {
                    continue;
                }
                let di = e.dir(d);
                if di.has_switch() && !di.has_volume() {
                    switches.push((d, e));
                }
                if di.has_enum() && !enums.iter().any(|(_, x)| std::ptr::eq(*x, e)) {
                    enums.push((d, e));
                }
            }
        }

        let avail_h = ui.available_height();
        let avail_w = ui.available_width();
        let n_cards = switches.len() + enums.len();
        let per_row = ((avail_w / 260.0).floor() as usize).max(1);
        let extras = if n_cards == 0 { 0.0 } else { 44.0 + n_cards.div_ceil(per_row) as f32 * 76.0 };
        let gap = 12.0;
        let group_w = |elems: &Vec<&Elem>, d: Dir| -> f32 {
            elems.iter().map(|e| self.strip_width(e, d) + gap).sum::<f32>()
        };
        let total_w: f32 = rows.iter().map(|(d, v)| group_w(v, *d)).sum::<f32>() + 40.0 * rows.len().saturating_sub(1) as f32;
        let combined = rows.len() > 1 && total_w <= avail_w - 8.0;
        let strip_h = if rows.len() > 1 && !combined {
            ((avail_h - extras - 110.0) / 2.0).clamp(300.0, 520.0)
        } else {
            (avail_h - extras - 64.0).clamp(320.0, 640.0)
        };

        ScrollArea::vertical().id_salt("mixer-scroll").auto_shrink([false, false]).show(ui, |ui| {
            if rows.is_empty() && switches.is_empty() && enums.is_empty() {
                empty_state(ui, &p, "No controls to show", "This device exposes no simple mixer controls for the selected view.");
                return;
            }
            if combined {
                ui.horizontal_top(|ui| {
                    for (i, (d, elems)) in rows.iter().enumerate() {
                        if i > 0 {
                            let (r, _) = ui.allocate_exact_size(vec2(28.0, strip_h + 44.0), Sense::hover());
                            ui.painter().vline(r.center().x, (r.top() + 8.0)..=(r.bottom() - 8.0), egui::Stroke::new(1.0, p.border));
                        }
                        ui.vertical(|ui| {
                            section_header(ui, &p, *d, elems.len());
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for e in elems {
                                    self.strip(ui, e, *d, strip_h);
                                }
                            });
                        });
                    }
                });
                ui.add_space(18.0);
            } else {
                for (d, elems) in &rows {
                    section_header(ui, &p, *d, elems.len());
                    ScrollArea::horizontal().id_salt(("strips", *d as u8)).auto_shrink([false, true]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for e in elems {
                                self.strip(ui, e, *d, strip_h);
                            }
                            ui.add_space(4.0);
                        });
                        ui.add_space(14.0);
                    });
                    ui.add_space(10.0);
                }
            }
            if !switches.is_empty() || !enums.is_empty() {
                small_header(ui, &p, "Switches & sources");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                    for (d, e) in &switches {
                        self.switch_card(ui, e, *d);
                    }
                    for (d, e) in &enums {
                        self.enum_card(ui, e, *d);
                    }
                });
                ui.add_space(16.0);
            }
        });
    }

    fn strip_width(&self, e: &Elem, d: Dir) -> f32 {
        let s = &*self.settings;
        let n = e.dir(d).volume_channels;
        let split = n > 1 && s.split.contains(&elem_key(self.dev_key, e, d));
        let ticks_w = if s.show_ticks { 34.0 } else { 0.0 };
        let w = if split { 32.0 + ticks_w + 46.0 * n as f32 } else { 78.0 + ticks_w };
        w.max(112.0)
    }

    fn context_menu(&mut self, resp: &egui::Response, e: &Elem, d: Dir) {
        let key = elem_key(self.dev_key, e, d);
        let di = e.dir(d).clone();
        let settings = &mut *self.settings;
        resp.context_menu(|ui| {
            ui.set_min_width(180.0);
            ui.label(egui::RichText::new(e.display_name()).font(font_semibold(13.0)));
            ui.separator();
            let hidden = settings.hidden.contains(&key);
            if ui.button(if hidden { "Show control" } else { "Hide control" }).clicked() {
                if hidden {
                    settings.hidden.remove(&key);
                } else {
                    settings.hidden.insert(key.clone());
                }
                ui.close();
            }
            if di.volume_channels > 1 {
                let split = settings.split.contains(&key);
                if ui.button(if split { "Link channels" } else { "Split channels" }).clicked() {
                    if split {
                        settings.split.remove(&key);
                    } else {
                        settings.split.insert(key.clone());
                    }
                    ui.close();
                }
            }
            if di.has_volume() {
                if di.has_db() && di.db_min <= 0 && di.db_max >= 0 && ui.button("Set to 0 dB").clicked() {
                    let pos = e.position_for_db(d, 0.0, settings.perceptual);
                    e.set_position(d, None, pos, settings.perceptual, 0);
                    ui.close();
                }
                if ui.button("Set to maximum").clicked() {
                    e.set_volume(d, None, di.vol_max);
                    ui.close();
                }
                if ui.button("Set to minimum").clicked() {
                    e.set_volume(d, None, di.vol_min);
                    ui.close();
                }
            }
        });
    }

    fn strip(&mut self, ui: &mut Ui, e: &Elem, d: Dir, h: f32) {
        let p = Palette::get(ui.ctx());
        let s = &*self.settings;
        let di = e.dir(d);
        let key = elem_key(self.dev_key, e, d);
        let n = di.volume_channels;
        let split = n > 1 && s.split.contains(&key);
        let ticks_w = if s.show_ticks { 34.0 } else { 0.0 };
        let w = self.strip_width(e, d);
        let (rect, card_resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        let id = Id::new(("strip", &key));
        let hov = ui.ctx().animate_bool_with_time(id.with("hov"), ui.rect_contains_pointer(rect), 0.15);
        let hidden = s.hidden.contains(&key);
        let inactive = !e.is_active();
        let mut ui_child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(id));
        let ui = &mut ui_child;
        if hidden || inactive {
            ui.multiply_opacity(0.45);
        }
        card_bg(ui.painter(), rect, &p, hov, 16);

        let perceptual = s.perceptual;
        let muted = match d {
            Dir::Playback => di.has_switch() && !e.switch_any(d),
            Dir::Capture => false,
        };

        // ---- header: centered name, up to two lines
        let inner = rect.shrink2(vec2(12.0, 12.0));
        let name = e.display_name();
        let mut job = egui::text::LayoutJob::simple(name.clone(), font_semibold(12.5), p.text, inner.width());
        job.halign = egui::Align::Center;
        job.wrap.max_rows = 2;
        job.wrap.break_anywhere = false;
        let galley = ui.painter().layout_job(job);
        let one_line = galley.rows.len() <= 1;
        let name_y = inner.top() + if one_line { 8.0 } else { 0.0 };
        ui.painter().galley(pos2(rect.center().x, name_y), galley, p.text);
        // ---- value readout
        let val_y = inner.top() + 46.0;
        let val = if split {
            let mut v: Vec<String> = Vec::new();
            for i in 0..n.min(2) {
                v.push(value_text(e, d, i, s).replace(" dB", ""));
            }
            v.join(" · ")
        } else {
            value_text(e, d, 0, s)
        };
        let val_color = if muted { p.text_faint } else { lerp_color(p.accent.a, p.accent.b, 0.35) };
        ui.painter().text(pos2(rect.center().x, val_y), Align2::CENTER_CENTER, val, font_semibold(if split { 12.0 } else { 15.0 }), val_color);

        // ---- faders
        let has_switch_row = di.has_switch();
        let bottom_reserved = if has_switch_row || n > 1 { 46.0 } else { 26.0 } + if split { 18.0 } else { 0.0 };
        let fader_area = Rect::from_min_max(pos2(inner.left(), val_y + 16.0), pos2(inner.right(), rect.bottom() - 12.0 - bottom_reserved));
        let track_len = fader_area.height() - widgets::THUMB_H - 8.0;
        let marks = tick_marks(e, d, s, track_len);
        let ticks = Ticks { marks: &marks, show_labels: s.show_ticks };
        let step = s.wheel_step / 100.0;

        let mut actions: Vec<(Option<usize>, Option<f32>, i32, bool)> = Vec::new();
        let tip = |ch: usize| {
            let ch_name = Elem::channel_long_name(e.dir(d).channels.get(ch).copied().unwrap_or(0));
            let mut t = format!("{} · {}\n{}", e.display_name(), d.label(), value_text(e, d, ch, s));
            if di.has_db() {
                t.push_str(&format!("\nRange {} … {}", fmt_db(di.db_min_f()), fmt_db(di.db_max_f())));
            }
            if n > 1 && split {
                t = format!("{t}\nChannel: {ch_name}");
            }
            t.push_str("\n\nDrag, scroll or double-click for 0 dB");
            t
        };
        if split {
            let fw = 46.0;
            let total = ticks_w + fw * n as f32;
            let x0 = fader_area.center().x - total / 2.0;
            for ch in 0..n {
                let (fx, fwid) = if ch == 0 { (x0, fw + ticks_w) } else { (x0 + ticks_w + fw * ch as f32, fw) };
                let fr = Rect::from_min_size(pos2(fx, fader_area.top()), vec2(fwid, fader_area.height()));
                let pos = e.position(d, ch, perceptual);
                let t = if ch == 0 { Some(&ticks) } else { Some(&Ticks { marks: &marks, show_labels: false }) };
                let out = widgets::fader(ui, fr, id.with(("f", ch)), pos, t, muted, step, || tip(ch));
                actions.push((Some(ch), out.value, out.round, out.reset));
                let ch_id = di.channels.get(ch).copied().unwrap_or(0);
                ui.painter().text(
                    pos2(fr.right() - fw / 2.0, fader_area.bottom() + 8.0),
                    Align2::CENTER_CENTER,
                    Elem::channel_name(ch_id),
                    font(10.0),
                    p.text_faint,
                );
            }
        } else {
            let fwid = 46.0 + ticks_w;
            let _ = ticks_w;
            let fr = Rect::from_center_size(fader_area.center(), vec2(fwid, fader_area.height()));
            let fr = fr.translate(vec2(-ticks_w / 2.0, 0.0));
            let pos = (0..n).map(|ch| e.position(d, ch, perceptual)).fold(0.0f32, f32::max);
            let out = widgets::fader(ui, fr, id.with("f"), pos, Some(&ticks), muted, step, || tip(0));
            actions.push((None, out.value, out.round, out.reset));
        }

        for (ch, value, round, reset) in actions {
            if let Some(v) = value {
                e.set_position(d, ch, v, perceptual, round);
            }
            if reset {
                if di.has_db() && di.db_min <= 0 && di.db_max >= 0 {
                    let pos = e.position_for_db(d, 0.0, perceptual);
                    e.set_position(d, ch, pos, perceptual, 0);
                } else {
                    e.set_position(d, ch, 0.75, perceptual, 0);
                }
            }
        }

        // ---- switches
        let mut sw_rect = Rect::from_min_max(pos2(inner.left(), rect.bottom() - 12.0 - 32.0), pos2(inner.right(), rect.bottom() - 12.0));
        if n > 1 {
            let r = Rect::from_min_max(pos2(sw_rect.right() - 30.0, sw_rect.top()), sw_rect.max);
            sw_rect.max.x -= if has_switch_row { 36.0 } else { 0.0 };
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(r));
            let resp = icon_button(&mut cui, r.size(), split, p.accent_mid(), "", |pt, rr, c| icon::link(pt, rr.expand(3.0), c, !split))
                .on_hover_text(if split { "Link channels" } else { "Split channels" });
            if resp.clicked() {
                if split {
                    self.settings.split.remove(&key);
                } else {
                    self.settings.split.insert(key.clone());
                }
            }
        }
        if has_switch_row {
            let tint = match d {
                Dir::Playback => p.danger,
                Dir::Capture => p.record,
            };
            if split && di.switch_channels > 1 {
                let bw = ((sw_rect.width() - 6.0 * (di.switch_channels - 1) as f32) / di.switch_channels as f32).min(46.0);
                let total = bw * di.switch_channels as f32 + 6.0 * (di.switch_channels - 1) as f32;
                let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(sw_rect.center(), vec2(total, 32.0))));
                cui.spacing_mut().item_spacing.x = 6.0;
                cui.horizontal(|ui| {
                    for ch in 0..di.switch_channels {
                        let on = e.switch(d, ch);
                        let active = match d {
                            Dir::Playback => !on,
                            Dir::Capture => on,
                        };
                        let r = icon_button(ui, vec2(bw, 32.0), active, tint, "", |pt, rr, c| match d {
                            Dir::Playback => icon::speaker(pt, rr, c, !on),
                            Dir::Capture => icon::record(pt, rr, c),
                        });
                        if r.clicked() {
                            e.set_switch(d, Some(ch), !on);
                        }
                    }
                });
            } else {
                let on = e.switch_any(d);
                let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(sw_rect));
                let (active, label) = match d {
                    Dir::Playback => (!on, if on { "Mute" } else { "Muted" }),
                    Dir::Capture => (on, if on { "Live" } else { "Off" }),
                };
                let r = icon_button(&mut cui, sw_rect.size(), active, tint, label, |pt, rr, c| match d {
                    Dir::Playback => icon::speaker(pt, rr, c, !on),
                    Dir::Capture => icon::record(pt, rr, c),
                });
                let r = r.on_hover_text(match d {
                    Dir::Playback => "Toggle mute",
                    Dir::Capture => "Toggle capture",
                });
                if r.clicked() {
                    e.set_switch(d, None, !on);
                }
            }
        } else if n <= 1 {
            let range = if di.has_db() {
                format!("{} … {}", fmt_db(di.db_min_f()).replace(" dB", ""), fmt_db(di.db_max_f()))
            } else {
                format!("{} steps", di.vol_max - di.vol_min)
            };
            let x = if n > 1 { (inner.left() + inner.right() - 36.0) / 2.0 } else { rect.center().x };
            ui.painter().text(pos2(x, sw_rect.center().y), Align2::CENTER_CENTER, range, font(10.0), p.text_faint);
        }
        if inactive {
            ui.painter().text(pos2(rect.center().x, val_y + 14.0), Align2::CENTER_CENTER, "inactive", font(9.5), p.text_faint);
        }

        self.context_menu(&card_resp, e, d);
    }

    fn switch_card(&mut self, ui: &mut Ui, e: &Elem, d: Dir) {
        let p = Palette::get(ui.ctx());
        let (rect, resp) = ui.allocate_exact_size(vec2(236.0, 64.0), Sense::click());
        let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() || ui.rect_contains_pointer(rect), 0.15);
        let key = elem_key(self.dev_key, e, d);
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        if self.settings.hidden.contains(&key) || !e.is_active() {
            cui.multiply_opacity(0.45);
        }
        card_bg(cui.painter(), rect, &p, hov, 14);
        let on = e.switch_any(d);
        let tint = if on { p.accent_mid() } else { p.text_faint };
        let ic = Rect::from_center_size(pos2(rect.left() + 32.0, rect.center().y), vec2(36.0, 36.0));
        cui.painter().circle_filled(ic.center(), 18.0, with_alpha(tint, 0.14));
        let ir = ic.shrink(9.0);
        match d {
            Dir::Playback => icon::speaker(cui.painter(), ir, tint, !on),
            Dir::Capture => icon::mic(cui.painter(), ir, tint),
        }
        let tx = rect.left() + 60.0;
        cui.painter().text(pos2(tx, rect.center().y - 9.0), Align2::LEFT_CENTER, truncate(&e.display_name(), 20), font_semibold(13.0), p.text);
        let mixed = e.dir(d).switch_channels > 1 && (0..e.dir(d).switch_channels).any(|i| e.switch(d, i) != on);
        let sub = format!("{} switch{}", d.label(), if mixed { " · mixed" } else { "" });
        cui.painter().text(pos2(tx, rect.center().y + 10.0), Align2::LEFT_CENTER, sub, font(11.0), p.text_dim);
        let tr = Rect::from_center_size(pos2(rect.right() - 34.0, rect.center().y), vec2(42.0, 24.0));
        let mut tui = cui.new_child(egui::UiBuilder::new().max_rect(tr));
        if widgets::toggle(&mut tui, on, None).clicked() || resp.clicked() {
            e.set_switch(d, None, !on);
        }
        self.context_menu(&resp, e, d);
    }

    fn enum_card(&mut self, ui: &mut Ui, e: &Elem, d: Dir) {
        let p = Palette::get(ui.ctx());
        let items = &e.enum_items;
        let short = items.len() <= 3 && items.iter().map(|s| s.len()).sum::<usize>() <= 24;
        let seg_w = if short {
            items.iter().map(|it| ui.painter().layout_no_wrap(it.clone(), font_semibold(12.0), p.text).size().x + 24.0).sum::<f32>().max(136.0) + 6.0
        } else {
            150.0
        };
        let name_w = ui.painter().layout_no_wrap(truncate(&e.display_name(), 26), font_semibold(13.0), p.text).size().x.max(110.0);
        let w = 16.0 + name_w + 20.0 + seg_w + 14.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 64.0), Sense::hover());
        let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), ui.rect_contains_pointer(rect), 0.15);
        let key = elem_key(self.dev_key, e, d);
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        if self.settings.hidden.contains(&key) || !e.is_active() {
            cui.multiply_opacity(0.45);
        }
        card_bg(cui.painter(), rect, &p, hov, 14);
        let cur = e.enum_item(0);
        cui.painter().text(pos2(rect.left() + 16.0, rect.top() + 20.0), Align2::LEFT_CENTER, truncate(&e.display_name(), 26), font_semibold(13.0), p.text);
        cui.painter().text(pos2(rect.left() + 16.0, rect.top() + 42.0), Align2::LEFT_CENTER, format!("{} selection", d.label()), font(11.0), p.text_dim);
        let ctrl = Rect::from_min_max(pos2(rect.right() - 14.0 - seg_w, rect.top() + 16.0), pos2(rect.right() - 14.0, rect.bottom() - 16.0));
        let mut sel = cur;
        let mut cui2 = cui.new_child(egui::UiBuilder::new().max_rect(ctrl).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if short && !items.is_empty() {
            let labels: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
            widgets::segmented(&mut cui2, Id::new(("enum", &key)), &mut sel, &labels, seg_w);
        } else {
            egui::ComboBox::from_id_salt(("enum", &key))
                .width(seg_w - 10.0)
                .selected_text(items.get(cur).cloned().unwrap_or_default())
                .show_ui(&mut cui2, |ui| {
                    for (i, it) in items.iter().enumerate() {
                        ui.selectable_value(&mut sel, i, it);
                    }
                });
        }
        if sel != cur {
            e.set_enum_item(None, sel);
        }
        self.context_menu(&resp, e, d);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

fn section_header(ui: &mut Ui, p: &Palette, d: Dir, count: usize) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        ui.painter().rect_filled(r, egui::CornerRadius::same(8), with_alpha(p.accent_mid(), 0.15));
        let ir = r.shrink(7.0);
        match d {
            Dir::Playback => icon::speaker(ui.painter(), ir, p.accent_mid(), false),
            Dir::Capture => icon::mic(ui.painter(), ir, p.accent_mid()),
        }
        ui.label(egui::RichText::new(d.label()).font(font_semibold(15.0)).color(p.text));
        widgets::badge(ui, &count.to_string(), p.text_dim, p.surface_hi);
    });
    ui.add_space(6.0);
}

pub fn small_header(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).font(font_semibold(11.0)).color(p.text_faint).extra_letter_spacing(1.2));
    ui.add_space(4.0);
}

pub fn empty_state(ui: &mut Ui, p: &Palette, title: &str, text: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 32.0, with_alpha(p.accent_mid(), 0.12));
        icon::sliders(ui.painter(), r.shrink(20.0), p.accent_mid());
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).font(font_semibold(16.0)));
        ui.label(egui::RichText::new(text).color(p.text_dim));
    });
}

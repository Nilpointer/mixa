//! Persistent user settings and the settings dialog.

use super::theme::{ACCENTS, Palette, font, font_bold, font_semibold, with_alpha};
use super::widgets::{self, gradient_rrect};
use eframe::egui::{self, Align2, CornerRadius, Id, RichText, Sense, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub accent: usize,
    /// Show levels in dB where available (otherwise percent).
    pub show_db: bool,
    /// Use alsamixer's perceptual volume mapping for slider positions.
    pub perceptual: bool,
    pub show_ticks: bool,
    /// 0 = all, 1 = playback, 2 = capture
    pub view: usize,
    /// Slider change per mouse wheel notch, in percent.
    pub wheel_step: f32,
    pub show_hidden: bool,
    pub hidden: BTreeSet<String>,
    pub split: BTreeSet<String>,
    pub device: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: true,
            accent: 0,
            show_db: true,
            perceptual: true,
            show_ticks: true,
            view: 0,
            wheel_step: 3.0,
            show_hidden: false,
            hidden: BTreeSet::new(),
            split: BTreeSet::new(),
            device: "default".into(),
        }
    }
}

fn row(ui: &mut Ui, title: &str, sub: &str, value: &mut bool) {
    let p = Palette::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).font(font_semibold(13.5)));
            ui.label(RichText::new(sub).font(font(11.5)).color(p.text_dim));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::toggle(ui, *value, None).clicked() {
                *value = !*value;
            }
        });
    });
    ui.add_space(6.0);
}

fn group(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let p = Palette::get(ui.ctx());
    ui.add_space(6.0);
    ui.label(RichText::new(title.to_uppercase()).font(font_semibold(11.0)).color(p.text_faint).extra_letter_spacing(1.2));
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(p.surface_hi)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
}

/// Shows the settings dialog. Returns false when it should close.
pub fn dialog(ctx: &egui::Context, s: &mut Settings) -> bool {
    let p = Palette::get(ctx);
    let mut open = true;
    let modal = egui::Modal::new(Id::new("settings-modal"))
        .backdrop_color(egui::Color32::from_black_alpha(if p.dark { 150 } else { 70 }))
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(18))
                .inner_margin(egui::Margin::same(22))
                .shadow(ctx.global_style().visuals.window_shadow),
        )
        .show(ctx, |ui| {
            ui.set_width(460.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Settings").font(font_bold(22.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = widgets::icon_button(ui, vec2(30.0, 30.0), false, p.accent_mid(), "", |pt, r, c| {
                        let d = r.width() * 0.3;
                        let st = Stroke::new(1.8, c);
                        pt.line_segment([r.center() - vec2(d, d), r.center() + vec2(d, d)], st);
                        pt.line_segment([r.center() + vec2(-d, d), r.center() + vec2(d, -d)], st);
                    });
                    if r.clicked() {
                        open = false;
                    }
                });
            });
            ui.add_space(4.0);
            let max_h = (ctx.content_rect().height() - 180.0).clamp(240.0, 640.0);
            egui::ScrollArea::vertical().max_height(max_h).min_scrolled_height(max_h).show(ui, |ui| {
                group(ui, "Appearance", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Theme").font(font_semibold(13.5)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mut t = if s.dark { 0 } else { 1 };
                            if widgets::segmented(ui, Id::new("theme-seg"), &mut t, &["Dark", "Light"], 160.0) {
                                s.dark = t == 0;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.label(RichText::new("Accent").font(font_semibold(13.5)));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        for (i, a) in ACCENTS.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(66.0, 52.0), Sense::click());
                            let sw = egui::Rect::from_center_size(r.center() - vec2(0.0, 8.0), vec2(30.0, 30.0));
                            if s.accent == i {
                                ui.painter().circle_stroke(sw.center(), 19.0, Stroke::new(2.0, a.b));
                            } else if resp.hovered() {
                                ui.painter().circle_stroke(sw.center(), 19.0, Stroke::new(1.0, p.border_hi));
                            }
                            gradient_rrect(ui.painter(), sw, 15.0, a.b, a.a, true);
                            ui.painter().text(r.center_bottom() - vec2(0.0, 6.0), Align2::CENTER_CENTER, a.name, font(10.5), p.text_dim);
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                s.accent = i;
                            }
                        }
                    });
                });
                group(ui, "Mixer", |ui| {
                    row(ui, "Show levels in dB", "Fall back to percent when a control has no dB scale", &mut s.show_db);
                    row(ui, "Perceptual volume curve", "Spread slider travel evenly over the audible range (like alsamixer)", &mut s.perceptual);
                    row(ui, "Scale marks", "Show dB / percent labels next to faders", &mut s.show_ticks);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new("Mouse wheel step").font(font_semibold(13.5)));
                            ui.label(RichText::new(format!("{:.0}% per notch", s.wheel_step)).font(font(11.5)).color(p.text_dim));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let pos = (s.wheel_step - 1.0) / 9.0;
                            if let (Some(v), _) = widgets::hslider(ui, Id::new("wheel-step"), 170.0, pos, true) {
                                s.wheel_step = (1.0 + v * 9.0).round();
                            }
                        });
                    });
                });
                group(ui, "Hidden controls", |ui| {
                    if s.hidden.is_empty() {
                        ui.label(RichText::new("Nothing hidden. Right-click a control to hide it.").color(p.text_dim));
                    }
                    let mut unhide = None;
                    for k in &s.hidden {
                        let mut parts = k.split('|');
                        let dev = parts.next().unwrap_or("");
                        let name = parts.next().unwrap_or("").replace(",0", "").replace(',', " ");
                        let dir = parts.next().unwrap_or("");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name).font(font_semibold(13.0)));
                            ui.label(RichText::new(format!("{dir} · {dev}")).font(font(11.0)).color(p.text_dim));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Show").clicked() {
                                    unhide = Some(k.clone());
                                }
                            });
                        });
                    }
                    if let Some(k) = unhide {
                        s.hidden.remove(&k);
                    }
                });
                group(ui, "About", |ui| {
                    ui.label(RichText::new(format!("mixa {}", env!("CARGO_PKG_VERSION"))).font(font_semibold(13.5)));
                    ui.label(
                        RichText::new(
                            "A modern ALSA mixer written in Rust — a reimagining of QasTools \
                             (QasMixer, QasHctl and QasConfig) by Sebastian Holtermann.",
                        )
                        .color(p.text_dim),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new("Shortcuts: Ctrl+1…4 pages · F5 reload · Ctrl+, settings · Ctrl+Q quit").font(font(11.5)).color(with_alpha(p.text_dim, 0.9)));
                });
            });
        });
    if modal.should_close() {
        open = false;
    }
    open
}

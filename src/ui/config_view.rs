//! "Configuration" page: browse the ALSA configuration tree (QasConfig equivalent).

use super::mixer_view::empty_state;
use super::theme::{Palette, font, font_mono, font_semibold, with_alpha};
use super::widgets::{badge, icon};
use crate::snd::config::{CfgNode, CfgValue};
use eframe::egui::{self, Color32, CornerRadius, Id, RichText, ScrollArea, Sense, Stroke, Ui, vec2};

#[derive(Default)]
pub struct State {
    pub search: String,
    pub expand_all: Option<bool>,
}

pub fn show(ui: &mut Ui, root: &CfgNode, st: &mut State) {
    let p = Palette::get(ui.ctx());
    let needle = st.search.to_lowercase();
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height() - 4.0);
            ScrollArea::both().id_salt("cfg-tree").auto_shrink([false, false]).show(ui, |ui| {
                let CfgValue::Compound(children) = &root.value else { return };
                let mut any = false;
                for c in children {
                    if c.matches(&needle) {
                        any = true;
                        node(ui, c, &needle, st.expand_all, Id::new("cfg"));
                    }
                }
                if !any {
                    empty_state(ui, &p, "No matches", "Nothing in the configuration matches your search");
                }
            });
        });
    st.expand_all = None;
}

fn type_color(p: &Palette, n: &CfgNode) -> Color32 {
    match n.value {
        CfgValue::Integer(_) => Color32::from_rgb(0x60, 0xa5, 0xfa),
        CfgValue::Real(_) => Color32::from_rgb(0xa7, 0x8b, 0xfa),
        CfgValue::Str(_) => if p.dark { Color32::from_rgb(0x86, 0xef, 0xac) } else { Color32::from_rgb(0x15, 0x80, 0x3d) },
        CfgValue::Pointer => p.text_faint,
        CfgValue::Compound(_) => p.accent_mid(),
    }
}

fn node(ui: &mut Ui, n: &CfgNode, needle: &str, expand: Option<bool>, parent: Id) {
    let p = Palette::get(ui.ctx());
    let id = parent.with(&n.id);
    match &n.value {
        CfgValue::Compound(children) => {
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
            if let Some(e) = expand {
                state.set_open(e);
            }
            if !needle.is_empty() {
                state.set_open(true);
            }
            state
                .show_header(ui, |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icon::tree(ui.painter(), r.shrink(2.0), with_alpha(p.accent_mid(), 0.8));
                    ui.label(RichText::new(&n.id).font(font_semibold(13.0)).color(p.text));
                    badge(ui, &children.len().to_string(), p.text_dim, p.surface_hi);
                })
                .body(|ui| {
                    for c in children {
                        if c.matches(needle) || n.id.to_lowercase().contains(needle) {
                            node(ui, c, needle, expand, id);
                        }
                    }
                });
        }
        _ => {
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.label(RichText::new(&n.id).font(font(13.0)).color(p.text_dim));
                let tc = type_color(&p, n);
                ui.label(RichText::new(n.value_text()).font(font_mono(12.5)).color(tc));
                badge(ui, n.type_name(), with_alpha(tc, 0.9), with_alpha(tc, 0.1));
            });
        }
    }
}

//! Application shell: sidebar navigation, device selection, page routing.

use super::settings::{self, Settings};
use super::theme::{self, Palette, font, font_bold, font_mono, font_semibold, lerp_color, with_alpha};
use super::widgets::{self, card_bg, gradient_rrect, icon};
use super::{config_view, controls_view, mixer_view};
use crate::snd::card::{self, CardInfo};
use crate::snd::config::CfgNode;
use crate::snd::hctl::HCtl;
use crate::snd::mixer::{Dir, Mixer};
use crate::snd::watch::Watcher;
use eframe::egui::{self, Align2, CornerRadius, Id, Key, Painter, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Mixer,
    Controls,
    Config,
    Device,
}

impl Page {
    const ALL: [Page; 4] = [Page::Mixer, Page::Controls, Page::Config, Page::Device];
    fn title(self) -> &'static str {
        match self {
            Page::Mixer => "Mixer",
            Page::Controls => "Controls",
            Page::Config => "Configuration",
            Page::Device => "Device",
        }
    }
    fn icon(self, p: &Painter, r: Rect, c: egui::Color32) {
        match self {
            Page::Mixer => icon::sliders(p, r, c),
            Page::Controls => icon::list(p, r, c),
            Page::Config => icon::tree(p, r, c),
            Page::Device => icon::chip(p, r, c),
        }
    }
}

pub struct MixaApp {
    settings: Settings,
    cards: Vec<CardInfo>,
    cards_checked: Instant,
    device: String,
    card: Option<CardInfo>,
    mixer: Result<Mixer, String>,
    hctl: Option<Result<HCtl, String>>,
    config: Option<Result<CfgNode, String>>,
    _watcher: Option<Watcher>,
    page: Page,
    custom_device: String,
    show_settings: bool,
    controls: controls_view::State,
    cfg: config_view::State,
    applied_theme: Option<(bool, usize)>,
    toast: Option<(String, Instant)>,
    shot: Option<String>,
    frame_no: u64,
}

impl MixaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, device_override: Option<String>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        if let Some(d) = device_override {
            settings.device = d;
        }
        let device = settings.device.clone();
        let mut app = MixaApp {
            settings,
            cards: card::list_cards(),
            cards_checked: Instant::now(),
            device: String::new(),
            card: None,
            mixer: Err(String::new()),
            hctl: None,
            config: None,
            _watcher: None,
            page: Page::Mixer,
            custom_device: String::new(),
            show_settings: false,
            controls: Default::default(),
            cfg: Default::default(),
            applied_theme: None,
            toast: None,
            shot: super::shot::requested(),
            frame_no: 0,
        };
        app.open_device(&cc.egui_ctx, &device);
        if app.mixer.is_err() && device != "default" {
            app.open_device(&cc.egui_ctx, "default");
        }
        if let Some(i) = super::shot::start_page() {
            app.page = Page::ALL[i.min(3)];
        }
        if super::shot::requested().is_some() {
            if std::env::var("MIXA_LIGHT").is_ok() {
                app.settings.dark = false;
            }
            app.show_settings = std::env::var("MIXA_SETTINGS").is_ok();
            if std::env::var("MIXA_SPLIT").is_ok()
                && let Ok(m) = &app.mixer {
                    let k = app.dev_key();
                    for e in &m.elems {
                        for d in Dir::ALL {
                            app.settings.split.insert(format!("{k}|{}|{:?}", e.key(), d));
                        }
                    }
                }
            if let Ok(q) = std::env::var("MIXA_SEARCH") {
                app.cfg.search = q.clone();
                app.controls.search = q;
            }
        }
        app
    }

    fn open_device(&mut self, ctx: &egui::Context, device: &str) {
        self._watcher = None;
        self.device = device.to_string();
        self.settings.device = device.to_string();
        self.card = card::card_info(device);
        self.mixer = Mixer::open(device);
        self.hctl = None;
        self.controls.selected = None;
        let c = ctx.clone();
        self._watcher = Watcher::start(device, move || c.request_repaint());
    }

    /// Stable identifier for per-device settings.
    fn dev_key(&self) -> String {
        self.card.as_ref().map(|c| c.id.clone()).unwrap_or_else(|| self.device.clone())
    }

    fn device_title(&self) -> String {
        match (&self.card, self.device.as_str()) {
            (_, "default") => "System default".into(),
            (Some(c), _) => c.name.clone(),
            (None, d) => d.to_string(),
        }
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        if let Ok(m) = &mut self.mixer
            && let Err(e) = m.poll() {
                self.mixer = Err(e);
                self._watcher = None;
            }
        if let Some(Ok(h)) = &mut self.hctl
            && let Err(e) = h.poll() {
                self.hctl = Some(Err(e));
            }
        // Detect hot-plugged cards.
        if self.cards_checked.elapsed() > Duration::from_secs(2) {
            self.cards_checked = Instant::now();
            let cards = card::list_cards();
            if cards != self.cards {
                let gone = self.device.starts_with("hw:") && !cards.iter().any(|c| c.device() == self.device);
                self.cards = cards;
                if gone {
                    self.toast("The selected card was removed — switched to the default device");
                    self.open_device(ctx, "default");
                } else if self.mixer.is_err() {
                    let d = self.device.clone();
                    self.open_device(ctx, &d);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (pages, reload, settings, quit) = ctx.input_mut(|i| {
            let mut pg = None;
            for (k, p) in [(Key::Num1, Page::Mixer), (Key::Num2, Page::Controls), (Key::Num3, Page::Config), (Key::Num4, Page::Device)] {
                if i.consume_key(egui::Modifiers::COMMAND, k) {
                    pg = Some(p);
                }
            }
            (
                pg,
                i.consume_key(egui::Modifiers::NONE, Key::F5),
                i.consume_key(egui::Modifiers::COMMAND, Key::Comma),
                i.consume_key(egui::Modifiers::COMMAND, Key::Q),
            )
        });
        if let Some(p) = pages {
            self.page = p;
        }
        if reload {
            self.reload(ctx);
        }
        if settings {
            self.show_settings = !self.show_settings;
        }
        if quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn reload(&mut self, ctx: &egui::Context) {
        let d = self.device.clone();
        self.cards = card::list_cards();
        self.open_device(ctx, &d);
        if self.page == Page::Config {
            self.config = None;
        }
        self.toast("Reloaded");
    }

    // ------------------------------------------------------------ sidebar

    fn sidebar(&mut self, ui: &mut Ui) {
        let p = Palette::get(ui.ctx());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
            icon::logo(ui.painter(), r, &p);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("mixa").font(font_bold(20.0)).color(p.text));
                ui.label(RichText::new("ALSA mixer").font(font(11.0)).color(p.text_dim));
            });
        });
        ui.add_space(22.0);

        nav_label(ui, &p, "Views");
        for pg in Page::ALL {
            if nav_item(ui, &p, pg.title(), self.page == pg, |pt, r, c| pg.icon(pt, r, c)).clicked() {
                self.page = pg;
            }
        }
        ui.add_space(18.0);
        nav_label(ui, &p, "Devices");

        let mut switch_to: Option<String> = None;
        let bottom_h = 64.0;
        let list_h = (ui.available_height() - bottom_h - 60.0).max(80.0);
        egui::ScrollArea::vertical().id_salt("dev-list").max_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
            if device_item(ui, &p, "System default", "default", self.device == "default").clicked() {
                switch_to = Some("default".into());
            }
            for c in &self.cards {
                let sub = if c.mixer_name.is_empty() { c.driver.clone() } else { c.mixer_name.clone() };
                let dev = c.device();
                if device_item(ui, &p, &c.name, &format!("{dev} · {sub}"), self.device == dev).clicked() {
                    switch_to = Some(dev);
                }
            }
            let is_custom = self.device != "default" && !self.cards.iter().any(|c| c.device() == self.device);
            if is_custom {
                device_item(ui, &p, &self.device, "custom device", true);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let te = egui::TextEdit::singleline(&mut self.custom_device)
                    .hint_text("Custom, e.g. hw:2")
                    .desired_width(ui.available_width() - 44.0)
                    .margin(vec2(10.0, 6.0));
                let r = ui.add(te);
                let go = widgets::icon_button(ui, vec2(36.0, 30.0), false, p.accent_mid(), "", |pt, r, c| {
                    pt.arrow(r.left_center(), vec2(r.width(), 0.0), Stroke::new(1.8, c));
                });
                if (go.clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))) && !self.custom_device.trim().is_empty() {
                    switch_to = Some(self.custom_device.trim().to_string());
                }
            });
        });
        if let Some(d) = switch_to
            && d != self.device {
                self.open_device(ui.ctx(), &d);
                if let Err(e) = &self.mixer {
                    let e = e.clone();
                    self.toast(e);
                }
            }

        // bottom row
        let rect = ui.max_rect();
        let row = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 40.0), rect.max);
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        bui.spacing_mut().item_spacing.x = 8.0;
        if widgets::icon_button(&mut bui, vec2(row.width() - 48.0, 38.0), false, p.accent_mid(), "Settings", icon::gear)
            .clicked()
        {
            self.show_settings = true;
        }
        let dark = self.settings.dark;
        let bg = lerp_color(p.inset, p.surface_hi, 0.6);
        if widgets::icon_button(&mut bui, vec2(40.0, 38.0), false, p.accent_mid(), "", |pt, r, c| {
            if dark { icon::sun(pt, r, c) } else { icon::moon(pt, r, c, bg) }
        })
        .on_hover_text(if dark { "Light theme" } else { "Dark theme" })
        .clicked()
        {
            self.settings.dark = !dark;
        }
    }

    // ------------------------------------------------------------ header

    fn header(&mut self, ui: &mut Ui) {
        let p = Palette::get(ui.ctx());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(self.page.title()).font(font_bold(26.0)).color(p.text));
                let mut sub = self.device_title();
                if let Some(c) = &self.card
                    && !c.mixer_name.is_empty() && self.device != "default" {
                        sub = format!("{sub} — {}", c.mixer_name);
                    }
                ui.label(RichText::new(sub).font(font(13.0)).color(p.text_dim));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                match self.page {
                    Page::Mixer => {
                        let sh = self.settings.show_hidden;
                        let n_hidden = self.settings.hidden.iter().filter(|k| k.starts_with(&format!("{}|", self.dev_key()))).count();
                        if n_hidden > 0
                            && widgets::icon_button(ui, vec2(36.0, 32.0), sh, p.accent_mid(), "", |pt, r, c| icon::eye(pt, r, c, !sh))
                                .on_hover_text(format!("{} hidden control{} — click to {}", n_hidden, if n_hidden == 1 { "" } else { "s" }, if sh { "hide" } else { "show" }))
                                .clicked()
                            {
                                self.settings.show_hidden = !sh;
                            }
                        widgets::segmented(ui, Id::new("view-seg"), &mut self.settings.view, &["All", "Playback", "Capture"], 270.0);
                    }
                    Page::Controls => {
                        search_box(ui, &mut self.controls.search, "Search controls");
                    }
                    Page::Config => {
                        if ui.button("Collapse all").clicked() {
                            self.cfg.expand_all = Some(false);
                        }
                        search_box(ui, &mut self.cfg.search, "Search configuration");
                        if widgets::icon_button(ui, vec2(36.0, 32.0), false, p.accent_mid(), "", icon::refresh)
                            .on_hover_text("Reload configuration")
                            .clicked()
                        {
                            self.config = None;
                        }
                    }
                    Page::Device => {}
                }
            });
        });
        ui.add_space(14.0);
    }

    // ------------------------------------------------------------ pages

    fn device_page(&mut self, ui: &mut Ui) {
        let p = Palette::get(ui.ctx());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // hero
            let w = ui.available_width();
            let (hero, _) = ui.allocate_exact_size(vec2(w, 132.0), Sense::hover());
            let painter = ui.painter();
            gradient_rrect(painter, hero, 20.0, with_alpha(p.accent.b, 0.85), with_alpha(p.accent.a, 0.85), false);
            for i in 0..28 {
                let x = hero.left() + 30.0 + i as f32 * 14.0 + (w - 460.0).max(0.0);
                let h = 12.0 + ((i as f32 * 1.7).sin().abs() * 0.7 + (i as f32 * 0.43).cos().abs() * 0.3) * 70.0;
                let r = Rect::from_center_size(pos2(x, hero.center().y), vec2(5.0, h));
                painter.rect_filled(r, CornerRadius::same(3), egui::Color32::from_white_alpha(46));
            }
            let ir = Rect::from_min_size(hero.left_top() + vec2(26.0, 34.0), vec2(64.0, 64.0));
            painter.rect_filled(ir, CornerRadius::same(18), egui::Color32::from_white_alpha(40));
            icon::chip(painter, ir.shrink(16.0), egui::Color32::WHITE);
            painter.text(pos2(ir.right() + 20.0, hero.center().y - 14.0), Align2::LEFT_CENTER, self.device_title(), font_bold(24.0), egui::Color32::WHITE);
            let sub = self.card.as_ref().map(|c| c.long_name.clone()).unwrap_or_else(|| format!("Device {}", self.device));
            painter.text(pos2(ir.right() + 20.0, hero.center().y + 16.0), Align2::LEFT_CENTER, sub, font(13.0), egui::Color32::from_white_alpha(220));
            ui.add_space(16.0);

            // stats
            let (pb, cap, sw) = match &self.mixer {
                Ok(m) => (
                    m.elems.iter().filter(|e| e.dir(Dir::Playback).has_volume()).count(),
                    m.elems.iter().filter(|e| e.dir(Dir::Capture).has_volume()).count(),
                    m.elems.iter().filter(|e| e.dirs.iter().any(|d| d.has_switch() || d.has_enum())).count(),
                ),
                Err(_) => (0, 0, 0),
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let cw = ((w - 36.0) / 4.0).max(120.0);
                stat(ui, &p, cw, "Playback faders", pb.to_string());
                stat(ui, &p, cw, "Capture faders", cap.to_string());
                stat(ui, &p, cw, "Switches & sources", sw.to_string());
                stat(ui, &p, cw, "Device string", self.device.clone());
            });
            ui.add_space(16.0);

            mixer_view::small_header(ui, &p, "Card information");
            egui::Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match &self.card {
                        None => {
                            ui.label(RichText::new("No card information available for this device (it may be a plugin such as PipeWire or PulseAudio).").color(p.text_dim));
                        }
                        Some(c) => {
                            egui::Grid::new("card-grid").num_columns(2).spacing(vec2(28.0, 12.0)).show(ui, |ui| {
                                for (k, v) in [
                                    ("Index", c.index.to_string()),
                                    ("ID", c.id.clone()),
                                    ("Driver", c.driver.clone()),
                                    ("Name", c.name.clone()),
                                    ("Long name", c.long_name.clone()),
                                    ("Mixer name", c.mixer_name.clone()),
                                    ("Components", c.components.clone()),
                                ] {
                                    ui.label(RichText::new(k).font(font(12.0)).color(p.text_faint));
                                    ui.label(RichText::new(v).font(font_mono(12.5)));
                                    ui.end_row();
                                }
                            });
                        }
                    }
                });
            ui.add_space(16.0);
            mixer_view::small_header(ui, &p, "All cards");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                let cards = self.cards.clone();
                for c in cards {
                    let (r, resp) = ui.allocate_exact_size(vec2(300.0, 84.0), Sense::click());
                    let hov = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                    card_bg(ui.painter(), r, &p, hov, 14);
                    let sel = self.device == c.device();
                    if sel {
                        ui.painter().rect_stroke(r, CornerRadius::same(14), Stroke::new(1.5, p.accent_mid()), egui::StrokeKind::Inside);
                    }
                    ui.painter().text(r.left_top() + vec2(16.0, 22.0), Align2::LEFT_CENTER, &c.name, font_semibold(14.0), p.text);
                    ui.painter().text(r.left_top() + vec2(16.0, 44.0), Align2::LEFT_CENTER, &c.mixer_name, font(12.0), p.text_dim);
                    ui.painter().text(r.left_top() + vec2(16.0, 64.0), Align2::LEFT_CENTER, format!("{} · {}", c.device(), c.driver), font_mono(11.0), p.text_faint);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !sel {
                        self.open_device(ui.ctx(), &c.device());
                    }
                }
            });
        });
    }

    fn error_card(&mut self, ui: &mut Ui, msg: &str) {
        let p = Palette::get(ui.ctx());
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 32.0, with_alpha(p.danger, 0.14));
            icon::speaker(ui.painter(), r.shrink(18.0), p.danger, true);
            ui.add_space(8.0);
            ui.label(RichText::new("Device unavailable").font(font_semibold(17.0)));
            ui.label(RichText::new(msg).color(p.text_dim));
            ui.add_space(10.0);
            if widgets::icon_button(ui, vec2(140.0, 36.0), true, p.accent_mid(), "Try again", icon::refresh).clicked() {
                self.reload(ui.ctx());
            }
        });
    }
}

impl eframe::App for MixaApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let theme_key = (self.settings.dark, self.settings.accent);
        if self.applied_theme != Some(theme_key) {
            theme::apply(&ctx, Palette::new(theme_key.0, theme_key.1));
            self.applied_theme = Some(theme_key);
        }
        let p = Palette::get(&ctx);
        self.frame_no += 1;
        if let Some(path) = &self.shot {
            super::shot::tick(&ctx, path, self.frame_no);
        }
        self.handle_events(&ctx);
        self.shortcuts(&ctx);

        egui::Panel::left("sidebar")
            .exact_size(256.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(egui::Margin::symmetric(16, 18)))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().vline(r.right() + 16.0, (r.top() - 18.0)..=(r.bottom() + 18.0), Stroke::new(1.0, p.border));
                self.sidebar(ui);
            });

        egui::CentralPanel::no_frame()
            .frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin { left: 28, right: 28, top: 22, bottom: 16 }))
            .show(ui, |ui| {
                self.header(ui);
                match self.page {
                    Page::Mixer => {
                        let dev_key = self.dev_key();
                        match &self.mixer {
                            Ok(m) => mixer_view::MixerPage { mixer: m, settings: &mut self.settings, dev_key: &dev_key }.show(ui),
                            Err(e) => {
                                let e = e.clone();
                                self.error_card(ui, &e)
                            }
                        }
                    }
                    Page::Controls => {
                        if self.hctl.is_none() {
                            self.hctl = Some(HCtl::open(&self.device));
                        }
                        match self.hctl.as_ref().unwrap() {
                            Ok(h) => controls_view::show(ui, h, &mut self.controls),
                            Err(e) => {
                                let e = e.clone();
                                self.error_card(ui, &e)
                            }
                        }
                        if let Some(err) = self.controls.error.take() {
                            self.toast(err);
                        }
                    }
                    Page::Config => {
                        if self.config.is_none() {
                            self.config = Some(crate::snd::config::load());
                        }
                        match self.config.as_ref().unwrap() {
                            Ok(root) => config_view::show(ui, root, &mut self.cfg),
                            Err(e) => {
                                let e = e.clone();
                                self.error_card(ui, &e)
                            }
                        }
                    }
                    Page::Device => self.device_page(ui),
                }
            });

        if self.show_settings && !settings::dialog(&ctx, &mut self.settings) {
            self.show_settings = false;
        }

        // toast
        if let Some((msg, t)) = &self.toast {
            let age = t.elapsed().as_secs_f32();
            if age > 3.5 {
                self.toast = None;
            } else {
                let a = (age * 6.0).min(1.0).min((3.5 - age) * 3.0);
                let screen = ctx.content_rect();
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("toast")));
                let g = painter.layout_no_wrap(msg.clone(), font_semibold(13.0), p.text);
                let r = Rect::from_center_size(pos2(screen.center().x + 128.0, screen.bottom() - 40.0 + (1.0 - a) * 12.0), g.size() + vec2(36.0, 20.0));
                let sh = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: egui::Color32::from_black_alpha((120.0 * a) as u8) };
                painter.add(sh.as_shape(r, CornerRadius::same(14)));
                painter.rect(r, CornerRadius::same(14), with_alpha(p.surface_hi, a), Stroke::new(1.0, with_alpha(p.border_hi, a)), egui::StrokeKind::Inside);
                painter.galley(r.center() - g.size() / 2.0, g, with_alpha(p.text, a));
                ctx.request_repaint();
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.shot.is_some() {
            return;
        }
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Palette::new(self.settings.dark, self.settings.accent).bg.to_normalized_gamma_f32()
    }
}

// ---------------------------------------------------------------- sidebar bits

fn nav_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).font(font_semibold(10.5)).color(p.text_faint).extra_letter_spacing(1.3));
    ui.add_space(2.0);
}

fn nav_item(ui: &mut Ui, p: &Palette, text: &str, active: bool, ic: impl FnOnce(&Painter, Rect, egui::Color32)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, active, 0.16);
    let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.1);
    let painter = ui.painter();
    if hov > 0.0 && t < 1.0 {
        painter.rect_filled(r, CornerRadius::same(10), with_alpha(p.surface_hi, hov * (1.0 - t)));
    }
    if t > 0.0 {
        gradient_rrect(painter, r, 10.0, with_alpha(p.accent.a, 0.28 * t), with_alpha(p.accent.b, 0.10 * t), false);
        let bar = Rect::from_min_size(pos2(r.left(), r.center().y - 9.0 * t), vec2(3.0, 18.0 * t));
        gradient_rrect(painter, bar, 1.5, p.accent.b, p.accent.a, true);
    }
    let c = lerp_color(lerp_color(p.text_dim, p.text, hov), p.text, t);
    let ic_c = lerp_color(c, lerp_color(p.accent.a, p.accent.b, 0.4), t);
    ic(painter, Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(16.0, 16.0)), ic_c);
    painter.text(pos2(r.left() + 42.0, r.center().y), Align2::LEFT_CENTER, text, font_semibold(13.5), c);
    ui.add_space(2.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn device_item(ui: &mut Ui, p: &Palette, title: &str, sub: &str, active: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, active, 0.16);
    let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.1);
    let painter = ui.painter();
    let bg = lerp_color(with_alpha(p.surface_hi, hov), p.surface, t);
    painter.rect_filled(r, CornerRadius::same(12), bg);
    if t > 0.0 {
        painter.rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0, with_alpha(p.accent_mid(), 0.6 * t)), egui::StrokeKind::Inside);
    }
    let dot = pos2(r.left() + 18.0, r.center().y);
    if t > 0.0 {
        widgets::glow(painter, dot, 12.0, p.accent.a, t);
    }
    painter.circle_filled(dot, 4.5, lerp_color(p.text_faint, p.accent.a, t));
    let clip = Rect::from_min_max(r.min, pos2(r.right() - 8.0, r.bottom()));
    let cp = painter.with_clip_rect(clip);
    cp.text(pos2(r.left() + 34.0, r.top() + 17.0), Align2::LEFT_CENTER, title, font_semibold(13.0), p.text);
    cp.text(pos2(r.left() + 34.0, r.top() + 34.0), Align2::LEFT_CENTER, sub, font(11.0), p.text_dim);
    ui.add_space(4.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn search_box(ui: &mut Ui, text: &mut String, hint: &str) {
    let p = Palette::get(ui.ctx());
    let w = 260.0;
    let (r, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(10), p.inset);
    ui.painter().rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    icon::search(ui.painter(), Rect::from_center_size(pos2(r.left() + 18.0, r.center().y), vec2(14.0, 14.0)), p.text_faint);
    let inner = Rect::from_min_max(pos2(r.left() + 32.0, r.top()), pos2(r.right() - 8.0, r.bottom()));
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    cui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .desired_width(inner.width()),
    );
}

fn stat(ui: &mut Ui, p: &Palette, w: f32, label: &str, value: String) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 78.0), Sense::hover());
    card_bg(ui.painter(), r, p, 0.0, 14);
    ui.painter().text(r.left_top() + vec2(16.0, 24.0), Align2::LEFT_CENTER, label, font(11.5), p.text_dim);
    ui.painter().text(r.left_top() + vec2(16.0, 52.0), Align2::LEFT_CENTER, value, font_bold(22.0), p.text);
}

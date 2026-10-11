//! Colors, fonts and global egui style.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    TextStyle, Visuals, epaint::text::FontInsert,
};


/// An accent gradient. `a` is the "low" end (bottom of faders), `b` the "high" end.
#[derive(Clone, Copy)]
pub struct Accent {
    pub name: &'static str,
    pub a: Color32,
    pub b: Color32,
}

pub const ACCENTS: [Accent; 5] = [
    Accent { name: "Aurora", a: Color32::from_rgb(0x22, 0xd3, 0xc5), b: Color32::from_rgb(0x8b, 0x5c, 0xf6) },
    Accent { name: "Sunset", a: Color32::from_rgb(0xf9, 0x73, 0x16), b: Color32::from_rgb(0xec, 0x48, 0x99) },
    Accent { name: "Ocean", a: Color32::from_rgb(0x38, 0xbd, 0xf8), b: Color32::from_rgb(0x63, 0x66, 0xf1) },
    Accent { name: "Lime", a: Color32::from_rgb(0x84, 0xcc, 0x16), b: Color32::from_rgb(0x10, 0xb9, 0x81) },
    Accent { name: "Rose", a: Color32::from_rgb(0xfb, 0x71, 0x85), b: Color32::from_rgb(0xa8, 0x55, 0xf7) },
];

/// All colors the custom widgets use. Kept in egui's temp data so any widget can fetch it.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub inset: Color32,
    pub border: Color32,
    pub border_hi: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Accent,
    pub danger: Color32,
    pub record: Color32,
    pub ok: Color32,
    pub thumb: Color32,
}

impl Palette {
    pub fn new(dark: bool, accent: usize) -> Palette {
        let accent = ACCENTS[accent % ACCENTS.len()];
        if dark {
            Palette {
                dark,
                bg: Color32::from_rgb(0x0d, 0x10, 0x17),
                sidebar: Color32::from_rgb(0x11, 0x15, 0x1e),
                surface: Color32::from_rgb(0x17, 0x1c, 0x27),
                surface_hi: Color32::from_rgb(0x1f, 0x26, 0x34),
                inset: Color32::from_rgb(0x0a, 0x0d, 0x13),
                border: Color32::from_rgb(0x26, 0x2d, 0x3b),
                border_hi: Color32::from_rgb(0x3a, 0x44, 0x57),
                text: Color32::from_rgb(0xe8, 0xec, 0xf3),
                text_dim: Color32::from_rgb(0x95, 0x9f, 0xb2),
                text_faint: Color32::from_rgb(0x5c, 0x66, 0x78),
                accent,
                danger: Color32::from_rgb(0xf4, 0x3f, 0x5e),
                record: Color32::from_rgb(0xef, 0x44, 0x44),
                ok: Color32::from_rgb(0x34, 0xd3, 0x99),
                thumb: Color32::from_rgb(0xf5, 0xf7, 0xfb),
            }
        } else {
            Palette {
                dark,
                bg: Color32::from_rgb(0xf3, 0xf5, 0xf9),
                sidebar: Color32::from_rgb(0xea, 0xee, 0xf4),
                surface: Color32::from_rgb(0xff, 0xff, 0xff),
                surface_hi: Color32::from_rgb(0xf6, 0xf8, 0xfb),
                inset: Color32::from_rgb(0xe3, 0xe8, 0xef),
                border: Color32::from_rgb(0xdd, 0xe3, 0xec),
                border_hi: Color32::from_rgb(0xc3, 0xcc, 0xd9),
                text: Color32::from_rgb(0x14, 0x1a, 0x26),
                text_dim: Color32::from_rgb(0x5a, 0x65, 0x78),
                text_faint: Color32::from_rgb(0x95, 0x9f, 0xb0),
                accent,
                danger: Color32::from_rgb(0xe1, 0x1d, 0x48),
                record: Color32::from_rgb(0xdc, 0x26, 0x26),
                ok: Color32::from_rgb(0x05, 0x96, 0x69),
                thumb: Color32::WHITE,
            }
        }
    }

    /// Accent color in the middle of the gradient (for text and outlines).
    pub fn accent_mid(&self) -> Color32 {
        lerp_color(self.accent.a, self.accent.b, 0.5)
    }

    pub fn get(ctx: &egui::Context) -> Palette {
        ctx.data(|d| d.get_temp::<Palette>(egui::Id::new("mixa-palette")))
            .unwrap_or_else(|| Palette::new(true, 0))
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let (a, b) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
    let l = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(0), l(1), l(2), l(3))
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn font_semibold(size: f32) -> FontId {
    FontId::new(size, semibold())
}
pub fn font_bold(size: f32) -> FontId {
    FontId::new(size, bold())
}
pub fn font_mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Loads Inter from the system when available; falls back to egui's bundled font.
pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    defs.families.insert(semibold(), fallback.clone());
    defs.families.insert(bold(), fallback);
    ctx.set_fonts(defs);

    let dirs = ["/usr/share/fonts/rsms-inter-fonts", "/usr/share/fonts/inter", "/usr/share/fonts/truetype/inter", "/usr/share/fonts/TTF", "/usr/share/fonts/opentype/inter"];
    let find = |file: &str| dirs.iter().find_map(|d| std::fs::read(format!("{d}/{file}")).ok());
    let add = |name: &str, files: &[&str], fam: FontFamily| {
        if let Some(bytes) = files.iter().find_map(|f| find(f)) {
            ctx.add_font(FontInsert::new(
                name,
                FontData::from_owned(bytes),
                vec![egui::epaint::text::InsertFontFamily {
                    family: fam,
                    priority: egui::epaint::text::FontPriority::Highest,
                }],
            ));
        }
    };
    add("inter-regular", &["Inter-Regular.ttf", "Inter-Regular.otf"], FontFamily::Proportional);
    add("inter-semibold", &["Inter-SemiBold.ttf", "Inter-SemiBold.otf"], semibold());
    add("inter-bold", &["InterDisplay-Bold.ttf", "Inter-Bold.ttf", "Inter-Bold.otf"], bold());
}

/// Applies the palette to egui's built-in widgets (combo boxes, text edits, scroll bars …).
pub fn apply(ctx: &egui::Context, p: Palette) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mixa-palette"), p));
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.inset;
    v.faint_bg_color = p.surface_hi;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(if p.dark { 140 } else { 40 }) };
    v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 30 }) };
    v.selection.bg_fill = with_alpha(p.accent_mid(), 0.35);
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.hyperlink_color = p.accent_mid();
    v.override_text_color = Some(p.text);
    let r = CornerRadius::same(10);
    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
    v.widgets.noninteractive.corner_radius = r;
    for (w, bg, st) in [
        (&mut v.widgets.inactive, p.surface_hi, p.border),
        (&mut v.widgets.hovered, lerp_color(p.surface_hi, p.border_hi, 0.4), p.border_hi),
        (&mut v.widgets.active, lerp_color(p.surface_hi, p.border_hi, 0.6), p.accent_mid()),
        (&mut v.widgets.open, p.surface_hi, p.accent_mid()),
    ] {
        w.bg_fill = bg;
        w.weak_bg_fill = bg;
        w.bg_stroke = Stroke::new(1.0, st);
        w.fg_stroke = Stroke::new(1.5, p.text);
        w.corner_radius = r;
        w.expansion = 0.0;
    }
    ctx.set_visuals(v);

    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.window_margin = Margin::same(20);
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
        s.text_styles.insert(TextStyle::Body, font(13.5));
        s.text_styles.insert(TextStyle::Button, font(13.5));
        s.text_styles.insert(TextStyle::Small, font(11.0));
        s.text_styles.insert(TextStyle::Heading, font_bold(22.0));
        s.text_styles.insert(TextStyle::Monospace, font_mono(12.5));
        s.animation_time = 0.14;
    });
}


//! Custom-painted controls: faders, toggle switches, segmented controls and icons.

use super::theme::{Palette, font, font_semibold, lerp_color, with_alpha};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Id, Mesh, Painter, Pos2, Rect, Response, Sense, Shadow, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------- gradients

fn rrect_outline(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() * 0.5).min(rect.height() * 0.5).max(0.0);
    let mut pts = Vec::with_capacity(40);
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -PI / 2.0),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), PI / 2.0),
        (pos2(rect.left() + r, rect.top() + r), PI),
    ];
    const SEG: usize = 8;
    for (c, a0) in corners {
        for i in 0..=SEG {
            let a = a0 + (i as f32 / SEG as f32) * (PI / 2.0);
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// Rounded rectangle filled with a gradient. `vertical`: top→bottom, else left→right.
pub fn gradient_rrect(painter: &Painter, rect: Rect, r: f32, from: Color32, to: Color32, vertical: bool) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let pts = rrect_outline(rect, r);
    let t_of = |p: Pos2| {
        if vertical {
            (p.y - rect.top()) / rect.height()
        } else {
            (p.x - rect.left()) / rect.width()
        }
    };
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), lerp_color(from, to, 0.5));
    for p in &pts {
        mesh.colored_vertex(*p, lerp_color(from, to, t_of(*p)));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
    // Thin outline in the blended color to soften the (non anti-aliased) mesh edge.
    let mut outline = pts.clone();
    outline.push(pts[0]);
    painter.add(Shape::line(outline, Stroke::new(0.6, with_alpha(lerp_color(from, to, 0.5), 0.5))));
}

/// Soft glow: concentric translucent circles.
pub fn glow(painter: &Painter, c: Pos2, r: f32, color: Color32, strength: f32) {
    for i in 0..6 {
        let t = i as f32 / 6.0;
        painter.circle_filled(c, r * (1.0 - t * 0.6), with_alpha(color, 0.05 * strength * (1.0 + t * 2.0)));
    }
}

/// Paints a card background with shadow, subtle gradient and border.
pub fn card_bg(painter: &Painter, rect: Rect, p: &Palette, hovered: f32, radius: u8) {
    let shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 90 } else { 18 }),
    };
    painter.add(shadow.as_shape(rect, CornerRadius::same(radius)));
    gradient_rrect(painter, rect, radius as f32, p.surface_hi, p.surface, true);
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.0, lerp_color(p.border, p.border_hi, hovered)),
        StrokeKind::Inside,
    );
}

// ---------------------------------------------------------------- fader

pub struct FaderOut {
    /// New position requested by the user (0..1).
    pub value: Option<f32>,
    /// Direction of the change (+1 up, -1 down) — used for rounding.
    pub round: i32,
    /// Double click: reset to a sensible default.
    pub reset: bool,
    pub response: Response,
}

pub struct Ticks<'a> {
    pub marks: &'a [(f32, String)],
    pub show_labels: bool,
}

pub const THUMB_H: f32 = 18.0;

/// A vertical fader painted into `rect`.
#[allow(clippy::too_many_arguments)]
pub fn fader(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    pos: f32,
    ticks: Option<&Ticks>,
    muted: bool,
    wheel_step: f32,
    tooltip: impl FnOnce() -> String,
) -> FaderOut {
    let p = Palette::get(ui.ctx());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let label_w = if ticks.is_some_and(|t| t.show_labels) { 34.0 } else { 0.0 };
    let track_x = rect.left() + label_w + (rect.width() - label_w) * 0.5;
    let top = rect.top() + THUMB_H * 0.5 + 4.0;
    let bot = rect.bottom() - THUMB_H * 0.5 - 4.0;
    let len = (bot - top).max(1.0);
    let y_of = |v: f32| bot - v.clamp(0.0, 1.0) * len;
    let v_of = |y: f32| ((bot - y) / len).clamp(0.0, 1.0);

    let drag_id = id.with("drag");
    let mut out = FaderOut { value: None, round: 0, reset: false, response: resp.clone() };

    // ---- interaction
    let pointer = resp.interact_pointer_pos();
    if resp.drag_started()
        && let Some(pp) = pointer {
            let thumb_y = y_of(pos);
            let off = if (pp.y - thumb_y).abs() <= THUMB_H { pp.y - thumb_y } else { 0.0 };
            ui.data_mut(|d| d.insert_temp(drag_id, off));
        }
    let mut dragging_pos = None;
    if resp.dragged() {
        if let Some(pp) = pointer {
            let off: f32 = ui.data(|d| d.get_temp(drag_id)).unwrap_or(0.0);
            let v = v_of(pp.y - off);
            dragging_pos = Some(v);
            if (v - pos).abs() > 1e-4 {
                out.value = Some(v);
                out.round = if v > pos { 1 } else { -1 };
            }
        }
    } else if resp.clicked()
        && let Some(pp) = pointer {
            let v = v_of(pp.y);
            out.value = Some(v);
            out.round = if v > pos { 1 } else { -1 };
        }
    if resp.double_clicked() {
        out.reset = true;
        out.value = None;
    }
    if resp.hovered() {
        let steps = wheel_steps(ui, id);
        if steps != 0.0 {
            let v = (pos + steps * wheel_step).clamp(0.0, 1.0);
            out.value = Some(v);
            out.round = steps.signum() as i32;
        }
    }

    // ---- painting
    let hot = ui.ctx().animate_bool_with_time(id.with("hot"), resp.hovered() || resp.dragged(), 0.15);
    let shown = match dragging_pos {
        Some(v) => {
            ui.ctx().animate_value_with_time(id.with("anim"), v, 0.0);
            v
        }
        None => ui.ctx().animate_value_with_time(id.with("anim"), pos, 0.18),
    };
    let painter = ui.painter_at(rect.expand(12.0));

    // ticks
    if let Some(t) = ticks {
        for (v, label) in t.marks {
            let y = y_of(*v);
            painter.line_segment(
                [pos2(track_x - 18.0, y), pos2(track_x - 14.0, y)],
                Stroke::new(1.0, p.text_faint),
            );
            if t.show_labels && !label.is_empty() {
                painter.text(pos2(track_x - 21.0, y), Align2::RIGHT_CENTER, label, font(9.5), p.text_faint);
            }
        }
    }

    // track
    let tw = 8.0;
    let track = Rect::from_min_max(pos2(track_x - tw / 2.0, top - 4.0), pos2(track_x + tw / 2.0, bot + 4.0));
    painter.rect_filled(track, CornerRadius::same(4), p.inset);
    painter.rect_stroke(
        track,
        CornerRadius::same(4),
        Stroke::new(1.0, with_alpha(Color32::BLACK, if p.dark { 0.35 } else { 0.06 })),
        StrokeKind::Inside,
    );

    // fill
    let ty = y_of(shown);
    let fill = Rect::from_min_max(pos2(track.left(), ty), track.max);
    let (ca, cb) = if muted {
        (p.text_faint, lerp_color(p.text_faint, p.text_dim, 0.5))
    } else {
        (p.accent.a, lerp_color(p.accent.a, p.accent.b, shown))
    };
    if fill.height() > 1.0 {
        gradient_rrect(&painter, fill, 4.0, cb, ca, true);
        if !muted {
            // soft halo around the filled track
            painter.rect_filled(fill.expand(3.0), CornerRadius::same(7), with_alpha(cb, 0.08 + 0.06 * hot));
        }
    }

    // thumb
    let thumb = Rect::from_center_size(pos2(track_x, ty), vec2(26.0, THUMB_H));
    if !muted {
        glow(&painter, thumb.center(), 26.0 + 6.0 * hot, cb, 0.6 + 0.8 * hot);
    }
    let sh = Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(if p.dark { 150 } else { 60 }) };
    painter.add(sh.as_shape(thumb, CornerRadius::same(9)));
    gradient_rrect(&painter, thumb, 9.0, p.thumb, lerp_color(p.thumb, p.border_hi, 0.35), true);
    painter.rect_stroke(thumb, CornerRadius::same(9), Stroke::new(1.0, with_alpha(Color32::BLACK, 0.12)), StrokeKind::Inside);
    let grip = if muted { p.text_faint } else { cb };
    painter.line_segment(
        [thumb.center() - vec2(5.0, 0.0), thumb.center() + vec2(5.0, 0.0)],
        Stroke::new(2.5, grip),
    );

    let resp = resp.on_hover_text_at_pointer(tooltip());
    out.response = resp;
    out
}

/// Number of wheel notches this frame while hovering a widget; consumes the scroll
/// so surrounding scroll areas don't move.
pub fn wheel_steps(ui: &Ui, id: Id) -> f32 {
    let acc_id = id.with("wheel-acc");
    let mut acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0);
    let mut steps = 0.0;
    ui.ctx().input_mut(|i| {
        for e in &i.events {
            if let egui::Event::MouseWheel { unit, delta, .. } = e {
                let d = if delta.y != 0.0 { delta.y } else { -delta.x };
                match unit {
                    egui::MouseWheelUnit::Point => acc += d / 40.0,
                    _ => acc += d,
                }
            }
        }
        i.smooth_scroll_delta = Vec2::ZERO;
    });
    if acc.abs() >= 1.0 {
        steps = acc.trunc();
        acc -= steps;
    }
    ui.data_mut(|d| d.insert_temp(acc_id, acc));
    steps
}

// ---------------------------------------------------------------- toggle switch

pub fn toggle(ui: &mut Ui, on: bool, color: Option<Color32>) -> Response {
    let p = Palette::get(ui.ctx());
    let size = vec2(42.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, on, 0.16);
    let painter = ui.painter();
    let r = rect.height() / 2.0;
    painter.rect_filled(rect, CornerRadius::same(r as u8), lerp_color(p.inset, p.surface_hi, 0.3));
    if t > 0.0 {
        let (a, b) = match color {
            Some(c) => (c, lerp_color(c, Color32::WHITE, 0.15)),
            None => (p.accent.a, p.accent.b),
        };
        let mut fill = rect;
        fill.max.x = rect.left() + rect.height() + (rect.width() - rect.height()) * t;
        gradient_rrect(painter, fill, r, with_alpha(a, t), with_alpha(b, t), false);
    }
    painter.rect_stroke(
        rect,
        CornerRadius::same(r as u8),
        Stroke::new(1.0, lerp_color(p.border, with_alpha(p.border, 0.0), t)),
        StrokeKind::Inside,
    );
    let cx = rect.left() + r + (rect.width() - 2.0 * r) * t;
    let c = pos2(cx, rect.center().y);
    painter.circle_filled(c + vec2(0.0, 1.5), r - 3.0, Color32::from_black_alpha(50));
    painter.circle_filled(c, r - 3.0, p.thumb);
    if resp.hovered() {
        painter.circle_stroke(c, r - 3.0, Stroke::new(1.0, with_alpha(p.accent_mid(), 0.6)));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- buttons

/// Rounded button with an optional painted icon. `active` fills it with `tint`.
pub fn icon_button(
    ui: &mut Ui,
    size: Vec2,
    active: bool,
    tint: Color32,
    label: &str,
    icon: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let p = Palette::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, active, 0.14);
    let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let painter = ui.painter();
    let r = (size.y / 2.0).min(10.0) as u8;
    let base = lerp_color(p.inset, p.surface_hi, 0.6 + 0.4 * hov);
    painter.rect_filled(rect, CornerRadius::same(r), lerp_color(base, with_alpha(tint, 0.22), t));
    painter.rect_stroke(
        rect,
        CornerRadius::same(r),
        Stroke::new(1.0, lerp_color(lerp_color(p.border, p.border_hi, hov), with_alpha(tint, 0.7), t)),
        StrokeKind::Inside,
    );
    let fg = lerp_color(if hov > 0.5 { p.text } else { p.text_dim }, lerp_color(tint, p.text, 0.25), t);
    if label.is_empty() {
        let s = size.y.min(size.x) * 0.55;
        icon(painter, Rect::from_center_size(rect.center(), Vec2::splat(s)), fg);
    } else {
        let icon_s = (size.y * 0.5).min(16.0);
        let galley = painter.layout_no_wrap(label.to_string(), font_semibold(11.5), fg);
        let total = icon_s + 6.0 + galley.size().x;
        let x0 = rect.center().x - total / 2.0;
        icon(painter, Rect::from_min_size(pos2(x0, rect.center().y - icon_s / 2.0), Vec2::splat(icon_s)), fg);
        painter.galley(pos2(x0 + icon_s + 6.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Segmented control with an animated selection pill. Returns true on change.
pub fn segmented(ui: &mut Ui, id: Id, selected: &mut usize, labels: &[&str], width: f32) -> bool {
    let p = Palette::get(ui.ctx());
    let h = 32.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), p.inset);
    painter.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, p.border), StrokeKind::Inside);
    let n = labels.len().max(1) as f32;
    let seg_w = (rect.width() - 6.0) / n;
    let x = ui.ctx().animate_value_with_time(id.with("sel"), *selected as f32, 0.18);
    let pill = Rect::from_min_size(pos2(rect.left() + 3.0 + x * seg_w, rect.top() + 3.0), vec2(seg_w, h - 6.0));
    let sh = Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 25 }) };
    painter.add(sh.as_shape(pill, CornerRadius::same(8)));
    gradient_rrect(painter, pill, 8.0, lerp_color(p.surface_hi, p.border_hi, 0.35), p.surface_hi, true);
    let mut changed = false;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + 3.0 + i as f32 * seg_w, rect.top()), vec2(seg_w, h));
        let resp = ui.interact(r, id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
        let near = 1.0 - (x - i as f32).abs().min(1.0);
        let c = lerp_color(if resp.hovered() { p.text } else { p.text_dim }, p.text, near);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *l, font_semibold(12.0), c);
    }
    changed
}

/// Small rounded label (e.g. "×2", "LFE").
pub fn badge(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font_semibold(10.5), fg);
    let size = galley.size() + vec2(12.0, 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

// ---------------------------------------------------------------- icons

pub fn arc(painter: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let n = 16;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    painter.add(Shape::line(pts, stroke));
}

pub mod icon {
    use super::*;

    fn sw(r: Rect) -> f32 {
        (r.width() / 10.0).clamp(1.3, 2.2)
    }

    pub fn speaker(p: &Painter, r: Rect, c: Color32, muted: bool) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        let cy = r.center().y;
        let x0 = r.left() + w * 0.06;
        let body = vec![
            pos2(x0, cy - w * 0.14),
            pos2(x0 + w * 0.2, cy - w * 0.14),
            pos2(x0 + w * 0.46, cy - w * 0.38),
            pos2(x0 + w * 0.46, cy + w * 0.38),
            pos2(x0 + w * 0.2, cy + w * 0.14),
            pos2(x0, cy + w * 0.14),
        ];
        p.add(Shape::convex_polygon(body, c, Stroke::NONE));
        let ax = x0 + w * 0.5;
        if muted {
            let cx = r.left() + w * 0.8;
            let d = w * 0.13;
            p.line_segment([pos2(cx - d, cy - d), pos2(cx + d, cy + d)], s);
            p.line_segment([pos2(cx - d, cy + d), pos2(cx + d, cy - d)], s);
        } else {
            arc(p, pos2(ax, cy), w * 0.2, -0.9, 0.9, s);
            arc(p, pos2(ax, cy), w * 0.38, -0.85, 0.85, s);
        }
    }

    pub fn mic(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        let cx = r.center().x;
        let head = Rect::from_center_size(pos2(cx, r.top() + w * 0.3), vec2(w * 0.34, w * 0.56));
        p.rect_filled(head, CornerRadius::same((w * 0.17) as u8), c);
        arc(p, pos2(cx, r.top() + w * 0.42), w * 0.3, 0.0, PI, s);
        p.line_segment([pos2(cx, r.top() + w * 0.72), pos2(cx, r.bottom() - w * 0.04)], s);
        p.line_segment([pos2(cx - w * 0.18, r.bottom() - w * 0.04), pos2(cx + w * 0.18, r.bottom() - w * 0.04)], s);
    }

    pub fn record(p: &Painter, r: Rect, c: Color32) {
        p.circle_filled(r.center(), r.width() * 0.3, c);
        p.circle_stroke(r.center(), r.width() * 0.45, Stroke::new(sw(r) * 0.8, c));
    }

    pub fn sliders(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        for (i, k) in [0.65f32, 0.3, 0.55].iter().enumerate() {
            let x = r.left() + r.width() * (0.2 + 0.3 * i as f32);
            p.line_segment([pos2(x, r.top() + 1.0), pos2(x, r.bottom() - 1.0)], s);
            let y = r.top() + r.height() * k;
            p.rect_filled(Rect::from_center_size(pos2(x, y), vec2(r.width() * 0.26, r.width() * 0.16)), CornerRadius::same(2), c);
        }
    }

    pub fn list(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        for i in 0..3 {
            let y = r.top() + r.height() * (0.2 + 0.3 * i as f32);
            p.circle_filled(pos2(r.left() + r.width() * 0.12, y), sw(r) * 0.9, c);
            p.line_segment([pos2(r.left() + r.width() * 0.32, y), pos2(r.right(), y)], s);
        }
    }

    pub fn tree(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        let a = pos2(r.left() + w * 0.2, r.top() + w * 0.18);
        p.line_segment([a, pos2(a.x, r.top() + w * 0.82)], s);
        for y in [0.5, 0.82] {
            let yy = r.top() + w * y;
            p.line_segment([pos2(a.x, yy), pos2(r.left() + w * 0.55, yy)], s);
            p.rect_filled(Rect::from_center_size(pos2(r.left() + w * 0.75, yy), vec2(w * 0.32, w * 0.2)), CornerRadius::same(2), c);
        }
        p.rect_filled(Rect::from_center_size(a, vec2(w * 0.32, w * 0.24)), CornerRadius::same(2), c);
    }

    pub fn chip(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        let inner = r.shrink(r.width() * 0.22);
        p.rect_stroke(inner, CornerRadius::same(3), s, StrokeKind::Middle);
        p.rect_filled(inner.shrink(inner.width() * 0.3), CornerRadius::same(1), c);
        for i in 0..3 {
            let t = 0.3 + 0.2 * i as f32;
            let x = r.left() + r.width() * t;
            let y = r.top() + r.height() * t;
            p.line_segment([pos2(x, r.top()), pos2(x, inner.top())], s);
            p.line_segment([pos2(x, inner.bottom()), pos2(x, r.bottom())], s);
            p.line_segment([pos2(r.left(), y), pos2(inner.left(), y)], s);
            p.line_segment([pos2(inner.right(), y), pos2(r.right(), y)], s);
        }
    }

    pub fn gear(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r) * 1.2, c);
        let w = r.width();
        p.circle_stroke(r.center(), w * 0.26, s);
        for i in 0..8 {
            let a = i as f32 * TAU / 8.0;
            let d = vec2(a.cos(), a.sin());
            p.line_segment([r.center() + d * w * 0.34, r.center() + d * w * 0.48], Stroke::new(sw(r) * 1.6, c));
        }
    }

    pub fn sun(p: &Painter, r: Rect, c: Color32) {
        let w = r.width();
        p.circle_filled(r.center(), w * 0.2, c);
        for i in 0..8 {
            let a = i as f32 * TAU / 8.0;
            let d = vec2(a.cos(), a.sin());
            p.line_segment([r.center() + d * w * 0.32, r.center() + d * w * 0.46], Stroke::new(sw(r), c));
        }
    }

    pub fn moon(p: &Painter, r: Rect, c: Color32, bg: Color32) {
        let w = r.width();
        p.circle_filled(r.center(), w * 0.38, c);
        p.circle_filled(r.center() + vec2(w * 0.18, -w * 0.14), w * 0.32, bg);
    }

    pub fn link(p: &Painter, r: Rect, c: Color32, linked: bool) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        let h = w * 0.36;
        let off = if linked { 0.0 } else { w * 0.08 };
        let a = Rect::from_center_size(r.center() - vec2(w * 0.17 + off, 0.0), vec2(w * 0.52, h));
        let b = Rect::from_center_size(r.center() + vec2(w * 0.17 + off, 0.0), vec2(w * 0.52, h));
        p.rect_stroke(a, CornerRadius::same((h / 2.0) as u8), s, StrokeKind::Middle);
        p.rect_stroke(b, CornerRadius::same((h / 2.0) as u8), s, StrokeKind::Middle);
        if !linked {
            p.line_segment([r.center() + vec2(0.0, -h), r.center() + vec2(0.0, h)], Stroke::new(sw(r), c));
        }
    }

    pub fn search(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        let cc = r.left_top() + vec2(w * 0.42, w * 0.42);
        p.circle_stroke(cc, w * 0.28, s);
        p.line_segment([cc + vec2(w * 0.2, w * 0.2), r.right_bottom() - vec2(w * 0.08, w * 0.08)], Stroke::new(sw(r) * 1.3, c));
    }

    pub fn refresh(p: &Painter, r: Rect, c: Color32) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        arc(p, r.center(), w * 0.34, -PI * 0.35, PI * 1.45, s);
        let tip = r.center() + vec2((-PI * 0.35).cos(), (-PI * 0.35).sin()) * w * 0.34;
        p.add(Shape::convex_polygon(
            vec![tip + vec2(w * 0.2, -w * 0.02), tip + vec2(-w * 0.04, -w * 0.18), tip + vec2(-w * 0.02, w * 0.12)],
            c,
            Stroke::NONE,
        ));
    }

    pub fn eye(p: &Painter, r: Rect, c: Color32, crossed: bool) {
        let s = Stroke::new(sw(r), c);
        let w = r.width();
        arc(p, r.center() + vec2(0.0, w * 0.3), w * 0.5, -PI * 0.8, -PI * 0.2, s);
        arc(p, r.center() - vec2(0.0, w * 0.3), w * 0.5, PI * 0.2, PI * 0.8, s);
        p.circle_filled(r.center(), w * 0.13, c);
        if crossed {
            p.line_segment([r.left_top() + vec2(w * 0.1, w * 0.1), r.right_bottom() - vec2(w * 0.1, w * 0.1)], s);
        }
    }

    /// The app logo: a gradient tile with three bars.
    pub fn logo(p: &Painter, r: Rect, pal: &Palette) {
        gradient_rrect(p, r, r.width() * 0.28, pal.accent.b, pal.accent.a, false);
        let w = r.width();
        for (i, h) in [0.34f32, 0.58, 0.42].iter().enumerate() {
            let x = r.left() + w * (0.3 + 0.2 * i as f32);
            let bar = Rect::from_center_size(pos2(x, r.center().y), vec2(w * 0.11, w * h));
            p.rect_filled(bar, CornerRadius::same((w * 0.055) as u8), Color32::WHITE);
        }
    }
}

// ---------------------------------------------------------------- horizontal slider

/// Horizontal slider for raw values; returns the new position (0..1) if changed.
pub fn hslider(ui: &mut Ui, id: Id, width: f32, pos: f32, enabled: bool) -> (Option<f32>, Response) {
    let p = Palette::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    let sense = if enabled { Sense::click_and_drag() } else { Sense::hover() };
    let resp = ui.interact(rect, id, sense);
    let l = rect.left() + 10.0;
    let r = rect.right() - 10.0;
    let v_of = |x: f32| ((x - l) / (r - l).max(1.0)).clamp(0.0, 1.0);
    let mut out = None;
    if (resp.dragged() || resp.clicked())
        && let Some(pp) = resp.interact_pointer_pos() {
            out = Some(v_of(pp.x));
        }
    if enabled && resp.hovered() {
        let steps = wheel_steps(ui, id);
        if steps != 0.0 {
            out = Some((pos + steps * 0.02).clamp(0.0, 1.0));
        }
    }
    let shown = if resp.dragged() {
        out.unwrap_or(pos)
    } else {
        ui.ctx().animate_value_with_time(id.with("anim"), pos, 0.15)
    };
    let hot = ui.ctx().animate_bool_with_time(id.with("hot"), resp.hovered() || resp.dragged(), 0.12);
    let painter = ui.painter();
    let cy = rect.center().y;
    let track = Rect::from_min_max(pos2(l - 3.0, cy - 3.0), pos2(r + 3.0, cy + 3.0));
    painter.rect_filled(track, CornerRadius::same(3), p.inset);
    let x = l + (r - l) * shown;
    let fill = Rect::from_min_max(track.min, pos2(x, track.max.y));
    if fill.width() > 1.0 && enabled {
        gradient_rrect(painter, fill, 3.0, p.accent.a, lerp_color(p.accent.a, p.accent.b, shown), false);
    }
    let c = pos2(x, cy);
    if enabled {
        glow(painter, c, 16.0 + 4.0 * hot, lerp_color(p.accent.a, p.accent.b, shown), 0.5 + 0.8 * hot);
    }
    painter.circle_filled(c + vec2(0.0, 1.5), 8.5, Color32::from_black_alpha(60));
    painter.circle_filled(c, 8.5, if enabled { p.thumb } else { p.text_faint });
    painter.circle_filled(c, 3.0, if enabled { lerp_color(p.accent.a, p.accent.b, shown) } else { p.inset });
    (out, resp)
}

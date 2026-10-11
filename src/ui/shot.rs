//! Developer aid: `MIXA_SCREENSHOT=out.png` captures the window after a few frames and exits.
//! `MIXA_PAGE=0..3` picks the page to show first.

use eframe::egui;

pub fn requested() -> Option<String> {
    std::env::var("MIXA_SCREENSHOT").ok()
}

pub fn start_page() -> Option<usize> {
    std::env::var("MIXA_PAGE").ok()?.parse().ok()
}

/// Call once per frame; returns true once the screenshot was saved.
pub fn tick(ctx: &egui::Context, path: &str, frame_no: u64) -> bool {
    if frame_no == 40 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }
    if frame_no < 300 {
        ctx.request_repaint();
    }
    let img = ctx.input(|i| {
        i.raw.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = img {
        let [w, h] = img.size;
        let mut raw = Vec::with_capacity(h * (w * 3 + 1));
        for y in 0..h {
            raw.push(0u8);
            for x in 0..w {
                let c = img.pixels[y * w + x];
                raw.extend_from_slice(&[c.r(), c.g(), c.b()]);
            }
        }
        let _ = std::fs::write(path, png(w as u32, h as u32, &raw));
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return true;
    }
    false
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Minimal PNG encoder using uncompressed deflate blocks.
fn png(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, c) in chunks.iter().enumerate() {
        z.push((i == chunks.len() - 1) as u8);
        let len = c.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(c);
    }
    z.extend_from_slice(&adler32(raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut chunk = |ty: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = ty.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

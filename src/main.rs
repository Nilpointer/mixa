mod snd;
mod ui;

use eframe::egui;

const HELP: &str = "\
mixa — a modern mixer for the Linux sound system ALSA

Usage: mixa [OPTIONS]

Options:
  -c, --card=NUMBER   select a sound card
  -D, --device=NAME   select a mixer device (e.g. hw:1, default)
  -h, --help          print this help
  -v, --version       print the version
";

fn parse_args() -> Option<String> {
    let mut args = std::env::args().skip(1);
    let mut device = None;
    while let Some(a) = args.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) => (f.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        match flag.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-v" | "--version" => {
                println!("mixa {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-c" | "--card" => {
                if let Some(v) = inline.or_else(|| args.next()) {
                    device = Some(format!("hw:{v}"));
                }
            }
            "-D" | "--device" => {
                device = inline.or_else(|| args.next());
            }
            _ if flag.starts_with("-D") && flag.len() > 2 => device = Some(flag[2..].to_string()),
            _ if flag.starts_with("-c") && flag.len() > 2 => device = Some(format!("hw:{}", &flag[2..])),
            _ => {
                eprintln!("Unknown option: {a}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    device
}

/// Window icon: the gradient logo tile, rendered in software.
fn app_icon() -> egui::IconData {
    let s = 128usize;
    let mut rgba = vec![0u8; s * s * 4];
    let (a, b) = ([0x8bu8, 0x5c, 0xf6], [0x22u8, 0xd3, 0xc5]);
    let radius = 34.0f32;
    for y in 0..s {
        for x in 0..s {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(radius, s as f32 - radius);
            let cy = fy.clamp(radius, s as f32 - radius);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let cov = (radius - d + 0.5).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let t = fx / s as f32;
            let mut px = [0u8; 4];
            for i in 0..3 {
                px[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8;
            }
            // three white bars
            for (k, h) in [0.34f32, 0.58, 0.42].iter().enumerate() {
                let bx = s as f32 * (0.3 + 0.2 * k as f32);
                let hw = s as f32 * 0.055;
                let hh = s as f32 * h / 2.0;
                let mid = s as f32 / 2.0;
                let dx = (fx - bx).abs() - hw;
                let dy = (fy - mid).abs() - (hh - hw);
                let dist = if dy > 0.0 { (dx.max(-hw).powi(2) + dy * dy).sqrt() - 0.0 } else { dx };
                let dist = if dy > 0.0 { ((fx - bx).powi(2) + dy * dy).sqrt() - hw } else { dist };
                let c = (0.5 - dist).clamp(0.0, 1.0);
                for v in px.iter_mut().take(3) {
                    *v = (*v as f32 + (255.0 - *v as f32) * c) as u8;
                }
            }
            px[3] = (cov * 255.0) as u8;
            rgba[(y * s + x) * 4..(y * s + x) * 4 + 4].copy_from_slice(&px);
        }
    }
    egui::IconData { rgba, width: s as u32, height: s as u32 }
}

fn main() -> eframe::Result {
    let device = parse_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("mixa")
            .with_app_id("mixa")
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([820.0, 560.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native("mixa", options, Box::new(move |cc| Ok(Box::new(ui::app::MixaApp::new(cc, device)))))
}

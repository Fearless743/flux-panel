use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

const TTL: Duration = Duration::from_secs(5 * 60);
const SECONDARY_TTL: Duration = Duration::from_secs(5 * 60);
const WIDTH: usize = 120;
const HEIGHT: usize = 40;

struct Entry {
    answer: String,
    expires_at: Instant,
    verified: bool,
}

pub struct Store {
    data: Mutex<HashMap<String, Entry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    pub id: String,
    pub captcha_id: String,
    pub captcha_image: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub background_image: String,
}

static DEFAULT_STORE: LazyLock<Store> = LazyLock::new(|| Store {
    data: Mutex::new(HashMap::new()),
});

pub fn default_store() -> &'static Store {
    &DEFAULT_STORE
}

impl Store {
    pub fn cleanup(&self) {
        let now = Instant::now();
        let mut data = self.data.lock().unwrap();
        data.retain(|_, v| now < v.expires_at);
    }

    pub fn generate(&self) -> AppResult<GenerateResult> {
        let mut rng = rand::thread_rng();
        let (answer, display, typ) = if rng.gen_range(0..2) == 0 {
            let mut a = rng.gen_range(1..=9);
            let mut b = rng.gen_range(1..=9);
            let op = ["+", "-", "×"][rng.gen_range(0..3)];
            match op {
                "+" => (format!("{}", a + b), format!("{}+{}=?", a, b), "ARITHMETIC"),
                "-" => {
                    if a < b {
                        std::mem::swap(&mut a, &mut b);
                    }
                    (format!("{}", a - b), format!("{}-{}=?", a, b), "ARITHMETIC")
                }
                _ => (format!("{}", a * b), format!("{}×{}=?", a, b), "ARITHMETIC"),
            }
        } else {
            const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
            let mut s = String::with_capacity(4);
            for _ in 0..4 {
                let idx = rng.gen_range(0..CHARS.len());
                s.push(CHARS[idx] as char);
            }
            (s.clone(), s, "CHAR")
        };

        let img_b64 = render_text_png(&display)?;
        let id = Uuid::new_v4().to_string();
        self.data.lock().unwrap().insert(
            id.clone(),
            Entry {
                answer: answer.to_lowercase(),
                expires_at: Instant::now() + TTL,
                verified: false,
            },
        );

        Ok(GenerateResult {
            id: id.clone(),
            captcha_id: id,
            captcha_image: img_b64.clone(),
            r#type: typ.to_string(),
            background_image: img_b64,
        })
    }

    pub fn verify(&self, id: &str, code: &str) -> bool {
        if id.is_empty() || code.is_empty() {
            return false;
        }
        let mut data = self.data.lock().unwrap();
        let Some(e) = data.get_mut(id) else {
            return false;
        };
        if Instant::now() >= e.expires_at {
            data.remove(id);
            return false;
        }
        if code.trim().to_lowercase() != e.answer {
            return false;
        }
        e.verified = true;
        e.expires_at = Instant::now() + SECONDARY_TTL;
        e.answer.clear();
        true
    }

    pub fn secondary(&self, id: &str) -> bool {
        if id.is_empty() {
            return false;
        }
        let mut data = self.data.lock().unwrap();
        let Some(e) = data.get(id) else {
            return false;
        };
        if Instant::now() >= e.expires_at || !e.verified {
            data.remove(id);
            return false;
        }
        data.remove(id);
        true
    }
}

pub fn start_cleanup() {
    tokio::spawn(async {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            DEFAULT_STORE.cleanup();
        }
    });
}

fn render_text_png(text: &str) -> AppResult<String> {
    let mut rng = rand::thread_rng();
    let mut img = vec![0u8; WIDTH * HEIGHT * 4];
    for px in img.chunks_exact_mut(4) {
        px[0] = 245;
        px[1] = 247;
        px[2] = 250;
        px[3] = 255;
    }

    // 噪点
    for _ in 0..80 {
        let x = rng.gen_range(0..WIDTH);
        let y = rng.gen_range(0..HEIGHT);
        let r = rng.gen_range(0..200);
        let g = rng.gen_range(0..200);
        let b = rng.gen_range(0..200);
        set_px(&mut img, x, y, r, g, b, 255);
    }
    // 干扰线
    for _ in 0..3 {
        let r = rng.gen_range(0..180);
        let g = rng.gen_range(0..180);
        let b = rng.gen_range(0..180);
        let x1 = rng.gen_range(0..WIDTH);
        let y1 = rng.gen_range(0..HEIGHT);
        let x2 = rng.gen_range(0..WIDTH);
        let y2 = rng.gen_range(0..HEIGHT);
        draw_line(&mut img, x1 as i32, y1 as i32, x2 as i32, y2 as i32, r, g, b);
    }

    // 5x7 点阵
    let mut x = 10i32;
    for ch in text.chars() {
        let glyph = glyph(ch);
        draw_glyph(&mut img, x, 8, glyph, 30, 30, 30);
        x += 14;
    }

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| AppError::new(e.to_string()))?;
        writer
            .write_image_data(&img)
            .map_err(|e| AppError::new(e.to_string()))?;
    }
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(buf)
    ))
}

fn set_px(img: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let idx = (y * WIDTH + x) * 4;
    img[idx] = r;
    img[idx + 1] = g;
    img[idx + 2] = b;
    img[idx + 3] = a;
}

fn draw_line(img: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, r: u8, g: u8, b: u8) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 > x1 { -1 } else { 1 };
    let sy = if y0 > y1 { -1 } else { 1 };
    let mut x = x0;
    let mut y = y0;
    let mut err = dx + dy;
    loop {
        if x >= 0 && y >= 0 {
            set_px(img, x as usize, y as usize, r, g, b, 255);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_glyph(img: &mut [u8], ox: i32, oy: i32, rows: [u8; 7], r: u8, g: u8, b: u8) {
    let scale = 2;
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) != 0 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = ox + col * scale + dx;
                        let y = oy + row as i32 * scale + dy;
                        if x >= 0 && y >= 0 {
                            set_px(img, x as usize, y as usize, r, g, b, 255);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_returns_png_data_uri() {
        let r = default_store().generate().unwrap();
        assert_eq!(r.id, r.captcha_id);
        assert!(r.captcha_image.starts_with("data:image/png;base64,"));
        assert!(!r.captcha_image.ends_with(','));
        assert!(r.r#type == "ARITHMETIC" || r.r#type == "CHAR");
        assert!(!default_store().verify(&r.id, "definitely-wrong"));
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '×' => [0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x00],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        _ => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
    }
}

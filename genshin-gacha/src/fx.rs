//! Software-rendered cinematic for the wish build-up: a 3D perspective
//! starfield "warp", a convergence, and a colourful rarity burst. Frames are
//! returned as PNG bytes and blitted through the Kitty graphics protocol.

use std::sync::{Arc, Mutex, OnceLock};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};

use crate::model::{Rarity, Rgb};

// Smaller + fewer frames than before → far less to render and transmit, so the
// wish starts almost immediately and plays smoothly.
const W: u32 = 460;
const H: u32 = 288;
const STARS: usize = 120;
const FRAMES: usize = 36;

/// Cached frames per rarity (index 0=3★, 1=4★, 2=5★) so a wish only ever renders
/// the cinematic once per session.
static CACHE: OnceLock<Mutex<[Option<Arc<Vec<Vec<u8>>>>; 3]>> = OnceLock::new();

pub fn wish_cinematic_cached(rarity: Rarity) -> Arc<Vec<Vec<u8>>> {
    let idx = (rarity.stars() - 3).min(2);
    let cache = CACHE.get_or_init(|| Mutex::new([None, None, None]));
    let mut guard = cache.lock().unwrap();
    if let Some(a) = &guard[idx] {
        return a.clone();
    }
    let frames = Arc::new(wish_cinematic(rarity));
    guard[idx] = Some(frames.clone());
    frames
}

// ---- small deterministic RNG so frames are reproducible --------------------
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn f(&mut self) -> f32 {
        (self.next() % 100000) as f32 / 100000.0
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

#[derive(Clone, Copy)]
struct Star {
    x: f32,
    y: f32,
    z: f32,
    pz: f32,
}

#[inline]
fn add_light(img: &mut RgbaImage, x: i32, y: i32, c: Rgb, a: f32) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 || a <= 0.0 {
        return;
    }
    let a = a.min(1.0);
    let px = img.get_pixel_mut(x as u32, y as u32);
    let add = |o: u8, n: u8| (o as f32 + n as f32 * a).clamp(0.0, 255.0) as u8;
    px[0] = add(px[0], c.r);
    px[1] = add(px[1], c.g);
    px[2] = add(px[2], c.b);
    px[3] = 255;
}

fn bloom(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, c: Rgb, intensity: f32) {
    if radius <= 0.0 {
        return;
    }
    let r2 = radius * radius;
    let x0 = (cx - radius).floor().max(0.0) as i32;
    let x1 = (cx + radius).ceil().min(W as f32) as i32;
    let y0 = (cy - radius).floor().max(0.0) as i32;
    let y1 = (cy + radius).ceil().min(H as f32) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d2 = dx * dx + dy * dy;
            if d2 < r2 {
                let f = 1.0 - (d2 / r2).sqrt();
                add_light(img, x, y, c, f * f * intensity);
            }
        }
    }
}

/// Additive anti-aliased-ish line (for warp streaks and burst rays).
fn line(img: &mut RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, thick: i32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = dx.abs().max(dy.abs()).max(1.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (x0 + dx * t) as i32;
        let y = (y0 + dy * t) as i32;
        for oy in -thick..=thick {
            for ox in -thick..=thick {
                let fall = 1.0 - ((ox * ox + oy * oy) as f32).sqrt() / (thick as f32 + 1.0);
                add_light(img, x + ox, y + oy, c, a * fall.max(0.0));
            }
        }
    }
}

fn background(img: &mut RgbaImage, top: Rgb, bottom: Rgb) {
    for y in 0..H {
        let t = y as f32 / H as f32;
        let c = top.lerp(bottom, t);
        for x in 0..W {
            img.put_pixel(x, y, image::Rgba([c.r, c.g, c.b, 255]));
        }
    }
}

fn encode(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    PngEncoder::new(&mut buf)
        .write_image(img.as_raw(), W, H, ExtendedColorType::Rgba8)
        .expect("png encode");
    buf
}

/// Render the whole build-up cinematic for the given best rarity. Returns one
/// PNG per frame (play at ~45ms each).
pub fn wish_cinematic(rarity: Rarity) -> Vec<Vec<u8>> {
    let rc = rarity.reveal_color();
    let seed = match rarity {
        Rarity::Five => 0xA1B2_C3D4,
        Rarity::Four => 0x1234_9876,
        Rarity::Three => 0x0F0F_1E2D,
    };
    let mut rng = Rng(seed);

    let mut stars: Vec<Star> = (0..STARS)
        .map(|_| {
            let z = rng.range(0.15, 3.0);
            Star { x: rng.range(-1.3, 1.3), y: rng.range(-1.3, 1.3), z, pz: z }
        })
        .collect();

    let cx = W as f32 / 2.0;
    let cy = H as f32 / 2.0;
    let fov = H as f32 * 0.9;

    let mut frames = Vec::with_capacity(FRAMES);

    for f in 0..FRAMES {
        let t = f as f32 / (FRAMES - 1) as f32;
        // Phase boundaries.
        let warp = (t / 0.55).min(1.0); // 0..1 during warp
        let burst = ((t - 0.72) / 0.28).clamp(0.0, 1.0); // 0..1 during burst
        let pull = ((t - 0.55) / 0.17).clamp(0.0, 1.0); // convergence

        let mut img = RgbaImage::new(W, H);
        let tint = rc.scale(0.10 * (0.3 + t));
        background(&mut img, Rgb::new(6, 7, 14).lerp(tint, 0.4), Rgb::new(2, 2, 6));

        // Advance and draw the starfield.
        let speed = 0.02 + 0.10 * warp * warp;
        for s in stars.iter_mut() {
            s.pz = s.z;
            // During warp, fly toward camera; during burst, blow outward fast.
            if burst > 0.0 {
                s.z -= speed * (1.0 + 6.0 * burst);
            } else {
                s.z -= speed;
                // Convergence pulls x,y toward centre.
                if pull > 0.0 {
                    s.x *= 1.0 - 0.10 * pull;
                    s.y *= 1.0 - 0.10 * pull;
                }
            }
            if s.z < 0.08 {
                s.z = rng.range(2.4, 3.0);
                s.x = rng.range(-1.3, 1.3);
                s.y = rng.range(-1.3, 1.3);
                s.pz = s.z;
            }

            let sx = cx + (s.x / s.z) * fov;
            let sy = cy + (s.y / s.z) * fov;
            let px = cx + (s.x / s.pz) * fov;
            let py = cy + (s.y / s.pz) * fov;

            let bright = (0.35 + 0.65 / s.z).min(1.6);
            // Star colour: white in warp, tinted toward rarity as it heats up.
            let col = Rgb::new(235, 240, 255).lerp(rc, (0.25 + 0.6 * t).min(0.9));
            let thick = if s.z < 0.6 { 1 } else { 0 };
            line(&mut img, px, py, sx, sy, col, bright * 0.9, thick);
        }

        // Central glow grows through the warp / convergence.
        let glow_r = 20.0 + 120.0 * t;
        let glow_i = 0.25 + 0.9 * pull.max(warp * 0.5);
        bloom(&mut img, cx, cy, glow_r, rc.lerp(Rgb::new(255, 255, 255), 0.4), glow_i);

        // The burst.
        if burst > 0.0 {
            let core = 1.0 - burst;
            // Blinding core early, fading.
            bloom(&mut img, cx, cy, 60.0 + 40.0 * burst, Rgb::new(255, 255, 255), 1.2 * (1.0 - burst * 0.7));
            // Expanding shockwave rings.
            for k in 0..3 {
                let phase = (burst - k as f32 * 0.12).max(0.0);
                if phase > 0.0 {
                    let r = phase * (W as f32 * 0.75);
                    ring(&mut img, cx, cy, r, rc, (1.0 - phase) * 0.9, 2 + k);
                }
            }
            // Radial light rays.
            let rays = if matches!(rarity, Rarity::Five) { 16 } else { 10 };
            for i in 0..rays {
                let ang = i as f32 / rays as f32 * std::f32::consts::TAU + burst * 0.4;
                let len = burst * (W as f32 * 0.55);
                let ex = cx + ang.cos() * len;
                let ey = cy + ang.sin() * len * 0.62;
                line(&mut img, cx, cy, ex, ey, rc.lerp(Rgb::new(255, 255, 255), 0.3), (1.0 - burst) * 0.8 + 0.1, 1);
            }
            // Spark particles.
            let mut prng = Rng(seed ^ 0x9E37);
            let sparks = if matches!(rarity, Rarity::Five) { 70 } else { 40 };
            for _ in 0..sparks {
                let ang = prng.range(0.0, std::f32::consts::TAU);
                let sp = prng.range(0.3, 1.0);
                let d = burst * sp * (W as f32 * 0.6);
                let x = cx + ang.cos() * d;
                let y = cy + ang.sin() * d * 0.7;
                add_light(&mut img, x as i32, y as i32, rc.lerp(Rgb::new(255, 255, 240), 0.5), (1.0 - burst) * 0.9);
                add_light(&mut img, x as i32 + 1, y as i32, rc, (1.0 - burst) * 0.5);
            }
            // A held wash of the rarity colour near the peak.
            let wash = (1.0 - (burst - 0.15).abs() * 3.0).clamp(0.0, 1.0);
            if wash > 0.0 {
                for y in 0..H {
                    for x in 0..W {
                        add_light(&mut img, x as i32, y as i32, rc, 0.30 * wash);
                    }
                }
            }
            let _ = core;
        }

        // Ease the tail down to black so the hand-off to the splash art is a
        // dissolve rather than a hard cut.
        if t > 0.80 {
            let f = 1.0 - ((t - 0.80) / 0.20).clamp(0.0, 1.0);
            let f = f * f;
            for p in img.pixels_mut() {
                p[0] = (p[0] as f32 * f) as u8;
                p[1] = (p[1] as f32 * f) as u8;
                p[2] = (p[2] as f32 * f) as u8;
            }
        }

        frames.push(encode(&img));
    }

    frames
}

fn ring(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, c: Rgb, a: f32, thick: i32) {
    if r < 1.0 {
        return;
    }
    let n = (r * 6.5) as usize + 12;
    for i in 0..n {
        let ang = i as f32 / n as f32 * std::f32::consts::TAU;
        let x = cx + ang.cos() * r;
        let y = cy + ang.sin() * r * 0.62; // slight vertical squash → perspective
        for w in -thick..=thick {
            add_light(img, x as i32, y as i32 + w, c, a);
        }
    }
}

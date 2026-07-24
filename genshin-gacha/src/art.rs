//! Splash-card art for each item.
//!
//! If `assets/portraits/<id>.png` exists it is used verbatim — drop real
//! artwork there. Otherwise a themed, procedurally-drawn "wish splash" card is
//! generated: a glowing rim-lit figure (or weapon) against an elemental bloom,
//! framed in the rarity color. Results are cached in memory.

use std::collections::HashMap;
use std::sync::Mutex;

use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, GenericImageView, ImageEncoder, RgbaImage};

use crate::model::{Item, ItemKind, Rgb, WeaponType};

const W: u32 = 560;
const H: u32 = 680;

static CACHE: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

/// PNG bytes for an item's card. Prefers a real asset, falls back to procedural.
pub fn card_png(item: &Item) -> Vec<u8> {
    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(bytes) = map.get(item.id) {
            return bytes.clone();
        }
    }

    let bytes = load_asset(item.id).unwrap_or_else(|| generate(item));

    let mut guard = CACHE.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(item.id.to_string(), bytes.clone());
    bytes
}

fn load_asset(id: &str) -> Option<Vec<u8>> {
    let path = format!("assets/portraits/{id}.png");
    let bytes = std::fs::read(&path).ok()?;
    // Validate it decodes as an image before handing it to the terminal.
    image::load_from_memory(&bytes).ok()?;
    Some(bytes)
}

/// A pre-generated detailed pixel-art sprite, if one exists.
fn load_pixel_asset(id: &str) -> Option<Vec<u8>> {
    let path = format!("assets/pixel/{id}.png");
    let bytes = std::fs::read(&path).ok()?;
    image::load_from_memory(&bytes).ok()?;
    Some(bytes)
}

/// Load a pixel sprite by raw asset id (e.g. an enemy's `enemy_slime`). Returns
/// None if no such sprite has been generated yet.
pub fn pixel_by_id(id: &str) -> Option<Vec<u8>> {
    load_pixel_asset(id)
}

/// Cached fade-in ladders for splash reveals.
static FADES: Mutex<Option<HashMap<String, std::sync::Arc<Vec<Vec<u8>>>>>> = Mutex::new(None);

/// Progressively brighter versions of an item's splash, from near-black to
/// full, so the reveal can dissolve in instead of popping. Downscaled (the card
/// is shown in a small cell box anyway) so encoding stays fast; cached per item.
pub fn fade_in_frames(item: &Item, steps: usize) -> std::sync::Arc<Vec<Vec<u8>>> {
    let key = format!("{}:{steps}", item.id);
    {
        let mut guard = FADES.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(v) = map.get(&key) {
            return v.clone();
        }
    }

    let base = card_png(item);
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(steps);
    if let Ok(img) = image::load_from_memory(&base) {
        // Match roughly the on-screen size; more than enough for the cell box.
        let small = img.resize(420, 620, FilterType::Triangle).to_rgba8();
        for s in 0..steps {
            // Ease-in so it blooms up gently.
            let t = (s + 1) as f32 / steps as f32;
            let f = t * t;
            let mut frame = small.clone();
            for p in frame.pixels_mut() {
                p[0] = (p[0] as f32 * f) as u8;
                p[1] = (p[1] as f32 * f) as u8;
                p[2] = (p[2] as f32 * f) as u8;
            }
            out.push(encode_rgba(&frame));
        }
    }
    let arc = std::sync::Arc::new(out);
    let mut guard = FADES.lock().unwrap();
    guard.get_or_insert_with(HashMap::new).insert(key, arc.clone());
    arc
}

/// A battle backdrop scene (`scene_goblin`, `scene_shadow_dragon`, …), darkened
/// so the sprites and UI panels stay readable on top. Cached after first load.
pub fn scene_by_id(id: &str) -> Option<Vec<u8>> {
    let key = format!("scene:{id}");
    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(b) = map.get(&key) {
            return Some(b.clone());
        }
    }
    let bytes = std::fs::read(format!("assets/scenes/{id}.png")).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        // Dim + cool it slightly so foreground art reads clearly.
        p[0] = (p[0] as f32 * 0.46) as u8;
        p[1] = (p[1] as f32 * 0.46) as u8;
        p[2] = (p[2] as f32 * 0.54) as u8;
    }
    let out = encode_rgba(&rgba);
    let mut guard = CACHE.lock().unwrap();
    guard.get_or_insert_with(HashMap::new).insert(key, out.clone());
    Some(out)
}

/// A decorated menu backdrop: a starry gradient with a few character sprites
/// composited into the lower corners. Rendered once and cached.
pub fn menu_backdrop(featured: &[&str]) -> Vec<u8> {
    let key = "menu_backdrop".to_string();
    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(b) = map.get(&key) {
            return b.clone();
        }
    }

    const MW: u32 = 960;
    const MH: u32 = 540;
    let mut img = RgbaImage::new(MW, MH);

    // Deep night gradient.
    let top = Rgb::new(9, 11, 26);
    let bot = Rgb::new(44, 26, 70);
    for y in 0..MH {
        let c = top.lerp(bot, y as f32 / MH as f32);
        for x in 0..MW {
            img.put_pixel(x, y, image::Rgba([c.r, c.g, c.b, 255]));
        }
    }

    // Soft central aura + stars.
    let add = |img: &mut RgbaImage, x: i32, y: i32, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x >= MW as i32 || y >= MH as i32 {
            return;
        }
        let p = img.get_pixel_mut(x as u32, y as u32);
        let f = |o: u8, n: u8| (o as f32 + n as f32 * a).clamp(0.0, 255.0) as u8;
        p[0] = f(p[0], c.r);
        p[1] = f(p[1], c.g);
        p[2] = f(p[2], c.b);
    };
    let (cx, cy, rad) = (MW as f32 * 0.5, MH as f32 * 0.42, MH as f32 * 0.85);
    for y in 0..MH as i32 {
        for x in 0..MW as i32 {
            let d = (((x as f32 - cx).powi(2)) + ((y as f32 - cy).powi(2))).sqrt() / rad;
            if d < 1.0 {
                let f = (1.0 - d).powi(2);
                add(&mut img, x, y, Rgb::new(120, 90, 210), f * 0.35);
            }
        }
    }
    let mut seed: u32 = 0x2468_ACE1;
    let mut rng = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..320 {
        let x = (rng() % MW) as i32;
        let y = (rng() % MH) as i32;
        let b = 0.25 + (rng() % 100) as f32 / 130.0;
        add(&mut img, x, y, Rgb::new(225, 230, 255), b);
        add(&mut img, x + 1, y, Rgb::new(225, 230, 255), b * 0.35);
    }

    // Two fighters facing off — splash art, composited into the outer thirds.
    // Each fades out toward the centre so the menu panel never hides them.
    let cw = MW * 2 / 5;
    for (n, id) in featured.iter().take(2).enumerate() {
        let left = n == 0;
        let Ok(bytes) = std::fs::read(format!("assets/portraits/{id}.png")) else {
            continue;
        };
        let Ok(dynimg) = image::load_from_memory(&bytes) else {
            continue;
        };
        let slab = dynimg.resize_to_fill(cw, MH, FilterType::Lanczos3).to_rgba8();
        // Mirror the right-hand fighter so the pair face each other.
        let slab = if left { slab } else { image::imageops::flip_horizontal(&slab) };
        let ox = if left { 0 } else { MW - cw };
        for (sx, sy, px) in slab.enumerate_pixels() {
            let t = sx as f32 / cw as f32;
            // Opaque on the outer half, fading to nothing toward the centre.
            let a = if left { (1.0 - t) * 2.0 } else { t * 2.0 }.clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let (dx, dy) = (ox + sx, sy);
            if dx >= MW || dy >= MH {
                continue;
            }
            let d = img.get_pixel_mut(dx, dy);
            // Slightly dimmed so the centred menu text stays dominant.
            let mix = |o: u8, s: u8| (o as f32 * (1.0 - a) + s as f32 * 0.88 * a) as u8;
            *d = image::Rgba([mix(d[0], px[0]), mix(d[1], px[1]), mix(d[2], px[2]), 255]);
        }
    }

    let bytes = encode_rgba(&img);
    let mut guard = CACHE.lock().unwrap();
    guard.get_or_insert_with(HashMap::new).insert(key, bytes.clone());
    bytes
}

/// A hand-authored pixel-art bust for the collection grid — an original sprite
/// drawn from each character's palette and hairstyle (inspired by, not scaled
/// from, the splash). Weapons fall back to a downscaled card.
pub fn pixel_png(item: &Item) -> Vec<u8> {
    let key = format!("px:{}", item.id);
    {
        let mut guard = CACHE.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(bytes) = map.get(&key) {
            return bytes.clone();
        }
    }

    let bytes = if item.is_character() {
        // Prefer a generated detailed pixel-art sprite; fall back to the
        // hand-drawn parametric one when the asset is absent.
        load_pixel_asset(item.id).unwrap_or_else(|| build_sprite(item))
    } else {
        load_pixel_asset(item.id).unwrap_or_else(|| {
            let base = card_png(item);
            build_pixel(&base).unwrap_or(base)
        })
    };

    let mut guard = CACHE.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(key, bytes.clone());
    bytes
}

fn build_pixel(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png).ok()?;
    let (w, h) = img.dimensions();
    let cw = (w as f32 * 0.76) as u32;
    let ch = (h as f32 * 0.60) as u32;
    let cx = (w - cw) / 2;
    let cy = (h as f32 * 0.05) as u32;
    let cropped = img.crop_imm(cx, cy, cw, ch);
    let grid_w = 56u32;
    let grid_h = (grid_w as f32 * ch as f32 / cw as f32).round().max(1.0) as u32;
    let small = cropped.resize_exact(grid_w, grid_h, FilterType::Triangle);
    let big = small.resize_exact(grid_w * 6, grid_h * 6, FilterType::Nearest);
    Some(encode_rgba(&big.to_rgba8()))
}

// ---------------------------------------------------------------------------
// Original pixel-art character sprites.
// ---------------------------------------------------------------------------

const PW: u32 = 36;
const PH: u32 = 44;
const OUTLINE: Rgb = Rgb::new(38, 30, 46);

#[derive(Clone, Copy)]
enum Hair {
    Long,
    Twintails,
    Ponytail,
    Short,
    Bob,
}

struct Palette {
    hair: Rgb,
    hair2: Rgb,
    eye: Rgb,
    skin: Rgb,
    skin2: Rgb,
    outfit: Rgb,
    outfit2: Rgb,
    accent: Rgb,
    style: Hair,
}

fn palette_for(item: &Item) -> Palette {
    let skin = Rgb::new(255, 223, 196);
    let skin2 = Rgb::new(232, 190, 165);
    let p = |hair, hair2, eye, outfit, outfit2, accent, style| Palette {
        hair,
        hair2,
        eye,
        skin,
        skin2,
        outfit,
        outfit2,
        accent,
        style,
    };
    match item.id {
        "yukihana" => p(
            Rgb::new(236, 242, 252), Rgb::new(196, 214, 238), Rgb::new(120, 196, 236),
            Rgb::new(226, 236, 248), Rgb::new(120, 176, 222), Rgb::new(198, 240, 255), Hair::Long,
        ),
        "guren" => p(
            Rgb::new(202, 52, 58), Rgb::new(150, 32, 46), Rgb::new(246, 182, 82),
            Rgb::new(188, 46, 50), Rgb::new(232, 192, 112), Rgb::new(255, 150, 90), Hair::Long,
        ),
        "yozora" => p(
            Rgb::new(150, 112, 222), Rgb::new(108, 80, 180), Rgb::new(208, 154, 255),
            Rgb::new(72, 72, 150), Rgb::new(228, 224, 255), Rgb::new(200, 170, 255), Hair::Twintails,
        ),
        "celestine" => p(
            Rgb::new(94, 194, 204), Rgb::new(58, 150, 166), Rgb::new(120, 194, 238),
            Rgb::new(150, 206, 226), Rgb::new(232, 246, 255), Rgb::new(198, 240, 255), Hair::Long,
        ),
        "mizuki" => p(
            Rgb::new(92, 152, 216), Rgb::new(58, 116, 182), Rgb::new(130, 198, 238),
            Rgb::new(96, 150, 200), Rgb::new(226, 236, 246), Rgb::new(198, 236, 255), Hair::Ponytail,
        ),
        "tsubaki" => p(
            Rgb::new(64, 122, 92), Rgb::new(42, 92, 66), Rgb::new(122, 204, 152),
            Rgb::new(92, 162, 122), Rgb::new(212, 240, 222), Rgb::new(150, 236, 190), Hair::Bob,
        ),
        "hotaru" => p(
            Rgb::new(162, 122, 226), Rgb::new(120, 86, 186), Rgb::new(206, 162, 255),
            Rgb::new(112, 92, 176), Rgb::new(226, 206, 255), Rgb::new(200, 170, 255), Hair::Short,
        ),
        _ => {
            let t = item.theme;
            p(t.mid, t.mid.scale(0.68), t.accent, t.mid.scale(0.8), t.accent, t.accent, Hair::Long)
        }
    }
}

fn ppx(img: &mut RgbaImage, x: i32, y: i32, c: Rgb) {
    if x >= 0 && y >= 0 && (x as u32) < PW && (y as u32) < PH {
        img.put_pixel(x as u32, y as u32, image::Rgba([c.r, c.g, c.b, 255]));
    }
}

fn prect(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            ppx(img, x, y, c);
        }
    }
}

fn poval(img: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, c: Rgb) {
    for y in (cy - ry)..=(cy + ry) {
        for x in (cx - rx)..=(cx + rx) {
            let dx = (x - cx) as f32 / rx as f32;
            let dy = (y - cy) as f32 / ry as f32;
            if dx * dx + dy * dy <= 1.0 {
                ppx(img, x, y, c);
            }
        }
    }
}

fn build_sprite(item: &Item) -> Vec<u8> {
    let s = palette_for(item);
    let mut spr = RgbaImage::new(PW, PH); // transparent layer
    let cx = 18i32;

    // Long hair falls behind the head first.
    let side_len = match s.style {
        Hair::Long => 33,
        Hair::Bob => 26,
        Hair::Ponytail => 24,
        Hair::Twintails => 22,
        Hair::Short => 22,
    };
    poval(&mut spr, cx, 16, 11, 12, s.hair); // back hair mass
    prect(&mut spr, 7, 14, 11, side_len, s.hair); // left fall
    prect(&mut spr, 25, 14, 29, side_len, s.hair); // right fall
    prect(&mut spr, 25, 14, 29, side_len, s.hair2); // shade the right fall

    match s.style {
        Hair::Twintails => {
            poval(&mut spr, 6, 25, 4, 7, s.hair);
            poval(&mut spr, 30, 25, 4, 7, s.hair);
            prect(&mut spr, 7, 19, 10, 20, s.outfit2); // ties
            prect(&mut spr, 26, 19, 29, 20, s.outfit2);
        }
        Hair::Ponytail => {
            poval(&mut spr, 31, 23, 3, 10, s.hair);
            prect(&mut spr, 25, 15, 28, 16, s.outfit2); // tie
        }
        _ => {}
    }

    // Face.
    poval(&mut spr, cx, 20, 7, 9, s.skin);
    prect(&mut spr, cx - 4, 27, cx + 4, 28, s.skin2); // jaw shadow

    // Bangs sit over the forehead, following the face width.
    for y in 11..=18 {
        let dy = (y - 16) as f32 / 12.0;
        let hw = (11.0 * (1.0 - dy * dy).max(0.0).sqrt()) as i32;
        let col = if y <= 12 { s.hair } else { s.hair };
        prect(&mut spr, cx - hw, y, cx + hw, y, col);
    }
    // Carve a little face back out under the bangs, with a centre part.
    poval(&mut spr, cx, 22, 6, 7, s.skin);
    prect(&mut spr, cx, 12, cx, 16, s.hair2); // centre part strand
    ppx(&mut spr, cx - 6, 17, s.hair); // side fringe tips
    ppx(&mut spr, cx + 6, 17, s.hair);

    // Eyes.
    prect(&mut spr, 12, 19, 15, 19, s.hair2); // left brow/lash
    prect(&mut spr, 21, 19, 24, 19, s.hair2); // right brow/lash
    prect(&mut spr, 12, 20, 14, 23, s.eye); // left eye
    prect(&mut spr, 22, 20, 24, 23, s.eye); // right eye
    ppx(&mut spr, 13, 20, Rgb::new(255, 255, 255)); // highlights
    ppx(&mut spr, 23, 20, Rgb::new(255, 255, 255));
    prect(&mut spr, 12, 23, 14, 23, s.hair2); // lower lash
    prect(&mut spr, 22, 23, 24, 23, s.hair2);

    // Blush, nose, mouth.
    ppx(&mut spr, 11, 24, Rgb::new(255, 180, 175));
    ppx(&mut spr, 25, 24, Rgb::new(255, 180, 175));
    ppx(&mut spr, cx, 25, s.skin2);
    prect(&mut spr, cx - 1, 26, cx + 1, 26, Rgb::new(198, 108, 108));

    // Neck.
    prect(&mut spr, cx - 2, 28, cx + 2, 30, s.skin);
    prect(&mut spr, cx - 2, 28, cx + 2, 28, s.skin2);

    // Shoulders / outfit (trapezoid widening downward).
    for y in 30..PH as i32 {
        let t = ((y - 30) as f32 / 10.0).min(1.0);
        let hw = (5.0 + 12.0 * t) as i32;
        prect(&mut spr, cx - hw, y, cx + hw, y, s.outfit);
    }
    // Collar in the secondary colour.
    for k in 0..5 {
        prect(&mut spr, cx - 3 - k, 30 + k, cx - 2 - k, 30 + k, s.outfit2);
        prect(&mut spr, cx + 2 + k, 30 + k, cx + 3 + k, 30 + k, s.outfit2);
    }
    prect(&mut spr, cx - 2, 30, cx + 2, 31, s.skin); // collarbone gap
    // Element gem.
    prect(&mut spr, cx - 1, 34, cx + 1, 36, s.accent);
    ppx(&mut spr, cx, 35, Rgb::new(255, 255, 255));

    // Outline every silhouette edge.
    outline(&mut spr);

    // Compose over a themed background with a rarity frame.
    let mut base = RgbaImage::new(PW, PH);
    let deep = item.theme.deep;
    for y in 0..PH {
        let t = y as f32 / PH as f32;
        let c = deep.lerp(deep.scale(1.9), t);
        for x in 0..PW {
            base.put_pixel(x, y, image::Rgba([c.r, c.g, c.b, 255]));
        }
    }
    let fr = item.rarity.reveal_color();
    for x in 0..PW as i32 {
        ppx_base(&mut base, x, 0, fr);
        ppx_base(&mut base, x, PH as i32 - 1, fr);
    }
    for y in 0..PH as i32 {
        ppx_base(&mut base, 0, y, fr);
        ppx_base(&mut base, PW as i32 - 1, y, fr);
    }
    for y in 0..PH {
        for x in 0..PW {
            let p = *spr.get_pixel(x, y);
            if p[3] > 0 {
                base.put_pixel(x, y, p);
            }
        }
    }

    let big = image::DynamicImage::ImageRgba8(base).resize_exact(PW * 7, PH * 7, FilterType::Nearest);
    encode_rgba(&big.to_rgba8())
}

fn ppx_base(img: &mut RgbaImage, x: i32, y: i32, c: Rgb) {
    if x >= 0 && y >= 0 && (x as u32) < PW && (y as u32) < PH {
        img.put_pixel(x as u32, y as u32, image::Rgba([c.r, c.g, c.b, 255]));
    }
}

/// Paint a dark outline on every empty pixel touching a drawn pixel.
fn outline(img: &mut RgbaImage) {
    let src = img.clone();
    for y in 0..PH as i32 {
        for x in 0..PW as i32 {
            if src.get_pixel(x as u32, y as u32)[3] != 0 {
                continue;
            }
            let neighbour = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                nx >= 0
                    && ny >= 0
                    && (nx as u32) < PW
                    && (ny as u32) < PH
                    && src.get_pixel(nx as u32, ny as u32)[3] != 0
            });
            if neighbour {
                ppx(img, x, y, OUTLINE);
            }
        }
    }
}

fn encode_rgba(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut buf = Vec::new();
    PngEncoder::new(&mut buf)
        .write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
        .expect("png encode");
    buf
}

fn generate(item: &Item) -> Vec<u8> {
    let mut img = RgbaImage::new(W, H);
    let theme = item.theme;
    let rarity_col = item.rarity.reveal_color();

    background(&mut img, theme.deep, theme.deep.scale(1.6));
    bloom(&mut img, W as f32 * 0.5, H as f32 * 0.42, W as f32 * 0.62, theme.mid, 0.85);
    bloom(&mut img, W as f32 * 0.5, H as f32 * 0.30, W as f32 * 0.30, rarity_col, 0.35);
    starfield(&mut img, theme.accent);

    match item.kind {
        ItemKind::Character { .. } => figure(&mut img, theme.deep, theme.accent, theme.mid),
        ItemKind::Weapon { weapon } => weapon_shape(&mut img, weapon, theme.deep, theme.accent),
    }

    // Soft vignette to focus the center.
    vignette(&mut img, theme.deep);
    frame(&mut img, rarity_col);

    encode(&img)
}

// ---------------------------------------------------------------------------
// Drawing primitives (all alpha-aware "over" compositing).
// ---------------------------------------------------------------------------

#[inline]
fn blend(img: &mut RgbaImage, x: i32, y: i32, c: Rgb, a: f32) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 || a <= 0.0 {
        return;
    }
    let a = a.clamp(0.0, 1.0);
    let px = img.get_pixel_mut(x as u32, y as u32);
    let mix = |old: u8, new: u8| (old as f32 * (1.0 - a) + new as f32 * a).round() as u8;
    px[0] = mix(px[0], c.r);
    px[1] = mix(px[1], c.g);
    px[2] = mix(px[2], c.b);
    px[3] = 255;
}

/// Additive light (for glows and rim lights).
#[inline]
fn add_light(img: &mut RgbaImage, x: i32, y: i32, c: Rgb, a: f32) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 || a <= 0.0 {
        return;
    }
    let a = a.clamp(0.0, 1.0);
    let px = img.get_pixel_mut(x as u32, y as u32);
    let add = |old: u8, new: u8| (old as f32 + new as f32 * a).clamp(0.0, 255.0) as u8;
    px[0] = add(px[0], c.r);
    px[1] = add(px[1], c.g);
    px[2] = add(px[2], c.b);
    px[3] = 255;
}

fn background(img: &mut RgbaImage, top: Rgb, bottom: Rgb) {
    for y in 0..H {
        let t = y as f32 / H as f32;
        let c = top.lerp(bottom, t * 0.9);
        for x in 0..W {
            let px = img.get_pixel_mut(x, y);
            *px = image::Rgba([c.r, c.g, c.b, 255]);
        }
    }
}

fn bloom(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, c: Rgb, intensity: f32) {
    let r2 = radius * radius;
    let x0 = (cx - radius).floor() as i32;
    let x1 = (cx + radius).ceil() as i32;
    let y0 = (cy - radius).floor() as i32;
    let y1 = (cy + radius).ceil() as i32;
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

fn starfield(img: &mut RgbaImage, c: Rgb) {
    // Deterministic pseudo-stars (no RNG dependency so cards are stable).
    let mut seed: u32 = 0x9E3779B9;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..140 {
        let x = (next() % W) as i32;
        let y = (next() % (H * 3 / 5)) as i32;
        let b = 0.3 + (next() % 100) as f32 / 160.0;
        add_light(img, x, y, c, b * 0.5);
        add_light(img, x + 1, y, c, b * 0.2);
        add_light(img, x, y + 1, c, b * 0.2);
    }
}

/// A rim-lit figure in a flowing gown — an abstract "character" silhouette
/// with shoulders, a waist and a flared hem so it reads as feminine.
fn figure(img: &mut RgbaImage, body: Rgb, rim: Rgb, glow: Rgb) {
    let cx = W as f32 * 0.5;
    let core = body.scale(0.5);
    let head_cy = 148.0;
    let head_r = 48.0;
    let rim_w = 8.0;

    // Body profile: half-width at key heights, linearly interpolated.
    // (y, half-width) from neck down to the hem.
    let profile: &[(f32, f32)] = &[
        (196.0, 20.0),  // neck
        (232.0, 34.0),  // trapezius
        (256.0, 78.0),  // shoulders
        (300.0, 62.0),  // bust
        (352.0, 50.0),  // waist (pinch)
        (410.0, 78.0),  // hips
        (520.0, 128.0), // gown
        (658.0, 182.0), // hem
    ];
    let half_width = |yf: f32| -> f32 {
        if yf < profile[0].0 || yf > profile[profile.len() - 1].0 {
            return -1.0;
        }
        for w in profile.windows(2) {
            let (y0, w0) = w[0];
            let (y1, w1) = w[1];
            if yf >= y0 && yf <= y1 {
                let t = (yf - y0) / (y1 - y0);
                return w0 + (w1 - w0) * t;
            }
        }
        -1.0
    };

    // Soft halo behind the head (suggests hair / aura).
    bloom(img, cx, head_cy, head_r * 2.4, glow.scale(0.6), 0.55);

    // Flowing hair: two tapering bands framing the head and shoulders.
    for y in (head_cy as i32 - 40)..320 {
        let yf = y as f32;
        let t = ((yf - (head_cy - 40.0)) / (320.0 - (head_cy - 40.0))).clamp(0.0, 1.0);
        let offset = head_r - 4.0 + t * 34.0; // sweeps outward as it falls
        let hair_hw = 16.0 + 8.0 * (1.0 - (t - 0.5).abs() * 2.0);
        for side in [-1.0f32, 1.0] {
            let hx = cx + side * offset;
            let x0 = (hx - hair_hw) as i32;
            let x1 = (hx + hair_hw) as i32;
            for x in x0..x1 {
                let edge = hair_hw - (x as f32 - hx).abs();
                if edge < 0.0 {
                    continue;
                }
                blend(img, x, y, core.scale(0.7), 0.9);
                if edge < 4.0 {
                    add_light(img, x, y, rim, (1.0 - edge / 4.0) * 0.7);
                }
            }
        }
    }

    // Body.
    for y in 190..H as i32 {
        let yf = y as f32;
        let hw = half_width(yf);
        if hw <= 0.0 {
            continue;
        }
        let x0 = (cx - hw).floor() as i32;
        let x1 = (cx + hw).ceil() as i32;
        for x in x0..x1 {
            let edge = hw - (x as f32 - cx).abs();
            if edge < 0.0 {
                continue;
            }
            let vt = ((yf - 232.0) / (658.0 - 232.0)).clamp(0.0, 1.0);
            let interior = core.lerp(body.scale(0.3), vt);
            blend(img, x, y, interior, 0.97);
            if edge < rim_w {
                add_light(img, x, y, rim, (1.0 - edge / rim_w) * 0.9);
            }
        }
    }

    // Head.
    for y in (head_cy - head_r) as i32..(head_cy + head_r) as i32 {
        for x in (cx - head_r) as i32..(cx + head_r) as i32 {
            let dx = x as f32 - cx;
            let dy = y as f32 - head_cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= head_r {
                blend(img, x, y, core, 0.97);
                if head_r - d < rim_w {
                    add_light(img, x, y, rim, (1.0 - (head_r - d) / rim_w) * 0.9);
                }
            }
        }
    }

    // A bright vision-like mote at the heart.
    bloom(img, cx, 320.0, 42.0, rim, 0.95);
}

fn weapon_shape(img: &mut RgbaImage, kind: WeaponType, body: Rgb, rim: Rgb) {
    let cx = W as f32 * 0.5;
    let core = body.scale(0.7);
    match kind {
        WeaponType::Bow => {
            // A glowing arc.
            let r = 210.0;
            let bx = cx + 60.0;
            let by = H as f32 * 0.45;
            for a in 0..900 {
                let ang = -1.15 + (a as f32 / 900.0) * 2.3;
                let x = bx + r * ang.cos() * -1.0;
                let y = by + r * ang.sin();
                for w in -4..4 {
                    add_light(img, x as i32 + w, y as i32, rim, 0.8);
                }
            }
            // Bowstring.
            for t in 0..900 {
                let f = t as f32 / 900.0;
                let x = bx - r * (-1.15f32).cos() * -1.0 + 0.0;
                let _ = x;
                let y = by + r * ((-1.15) + f * 2.3).sin();
                let sx = bx - r + 8.0;
                add_light(img, sx as i32, y as i32, rim.scale(0.7), 0.5);
            }
        }
        WeaponType::Catalyst => {
            // A floating orb with an orbiting ring.
            bloom(img, cx, H as f32 * 0.42, 150.0, rim, 0.9);
            for y in 0..H as i32 {
                for x in 0..W as i32 {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - H as f32 * 0.42;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < 96.0 {
                        blend(img, x, y, core, 0.9);
                        if 96.0 - d < 10.0 {
                            add_light(img, x, y, rim, (10.0 - (96.0 - d)) / 10.0);
                        }
                    }
                    if (d - 150.0).abs() < 3.0 {
                        add_light(img, x, y, rim, 0.8);
                    }
                }
            }
        }
        _ => {
            // Bladed weapon: a long rim-lit spike (sword / claymore / polearm).
            let top = 110.0;
            let bot = 560.0;
            let max_hw = match kind {
                WeaponType::Claymore => 46.0,
                WeaponType::Polearm => 20.0,
                _ => 30.0,
            };
            for y in top as i32..bot as i32 {
                let t = (y as f32 - top) / (bot - top);
                let hw = (max_hw * (1.0 - (t - 0.15).abs() * 1.1)).max(3.0);
                let x0 = (cx - hw) as i32;
                let x1 = (cx + hw) as i32;
                for x in x0..x1 {
                    let edge = hw - (x as f32 - cx).abs();
                    blend(img, x, y, core, 0.95);
                    if edge < 5.0 {
                        add_light(img, x, y, rim, 1.0 - edge / 5.0);
                    }
                }
            }
            // Guard / hilt near the bottom.
            for y in 560..600 {
                for x in (cx - 60.0) as i32..(cx + 60.0) as i32 {
                    blend(img, x, y, core.scale(0.8), 0.9);
                }
            }
            bloom(img, cx, 200.0, 90.0, rim, 0.7);
        }
    }
}

fn vignette(img: &mut RgbaImage, dark: Rgb) {
    let cx = W as f32 * 0.5;
    let cy = H as f32 * 0.5;
    let maxd = (cx * cx + cy * cy).sqrt();
    for y in 0..H {
        for x in 0..W {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt() / maxd;
            if d > 0.62 {
                let f = ((d - 0.62) / 0.38).clamp(0.0, 1.0);
                blend(img, x as i32, y as i32, dark.scale(0.3), f * 0.7);
            }
        }
    }
}

fn frame(img: &mut RgbaImage, c: Rgb) {
    let inset = 14i32;
    let thick = 3i32;
    let draw_rect = |img: &mut RgbaImage, col: Rgb, a: f32| {
        for x in inset..(W as i32 - inset) {
            for t in 0..thick {
                blend(img, x, inset + t, col, a);
                blend(img, x, H as i32 - inset - 1 - t, col, a);
            }
        }
        for y in inset..(H as i32 - inset) {
            for t in 0..thick {
                blend(img, inset + t, y, col, a);
                blend(img, W as i32 - inset - 1 - t, y, col, a);
            }
        }
    };
    draw_rect(img, c, 0.9);

    // Corner diamonds.
    let corners = [
        (inset + 2, inset + 2),
        (W as i32 - inset - 3, inset + 2),
        (inset + 2, H as i32 - inset - 3),
        (W as i32 - inset - 3, H as i32 - inset - 3),
    ];
    for (cxp, cyp) in corners {
        for dy in -6i32..=6 {
            for dx in -6i32..=6 {
                if dx.abs() + dy.abs() <= 6 {
                    add_light(img, cxp + dx, cyp + dy, c, 0.5);
                }
            }
        }
    }
}

fn encode(img: &RgbaImage) -> Vec<u8> {
    encode_rgba(img)
}

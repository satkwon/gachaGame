//! The theatrical layer: starfields, the streaking wish comet, the rarity
//! colour reveal, star ignition and the grand 5-star cutscene. Everything runs
//! in crossterm raw mode so key-presses can skip or advance.

use std::io::{Stdout, Write};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Print, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, queue};

use crate::art;
use crate::gacha::WishOutcome;
use crate::graphics;
use crate::model::{Item, Rarity, Rgb};

pub struct Stage {
    out: Stdout,
    pub w: u16,
    pub h: u16,
    pub fast: bool,
    pub bell: bool,
    img_cols: u16,
    img_rows: u16,
    stars: Vec<(u16, u16, f32)>,
    graphics: bool,
    /// Set when the player asks to skip straight to the results.
    skip_all: bool,
}

enum Input {
    /// Advance / skip the current beat (space, enter).
    Advance,
    /// Skip the whole remaining sequence, jump to results (esc, s).
    SkipAll,
    Other,
}

fn classify(k: KeyEvent) -> Input {
    match k.code {
        KeyCode::Esc | KeyCode::Char('s') | KeyCode::Char('S') => Input::SkipAll,
        KeyCode::Char(' ') | KeyCode::Enter => Input::Advance,
        _ => Input::Other,
    }
}

impl Stage {
    pub fn new(out: Stdout, fast: bool, bell: bool) -> Self {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        // A cheap deterministic scatter of stars.
        let mut stars = Vec::new();
        let mut seed: u32 = 0x1234_5678;
        let mut rng = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let count = (w as u32 * h as u32 / 14).max(20);
        for _ in 0..count {
            let x = (rng() % w as u32) as u16;
            let y = (rng() % h as u32) as u16;
            let phase = (rng() % 628) as f32 / 100.0;
            stars.push((x, y, phase));
        }
        // Size the portrait so image + up-to-9 label rows + prompt always fit,
        // keeping the card's ~0.82 aspect (cells are ~twice as tall as wide).
        let img_rows = (h.saturating_sub(11)).clamp(8, 20);
        let img_cols = ((img_rows as f32 * 1.63) as u16).clamp(10, w.saturating_sub(2));
        Stage {
            out,
            w,
            h,
            fast,
            bell,
            img_cols,
            img_rows,
            stars,
            graphics: graphics::supported(),
            skip_all: false,
        }
    }

    // ---- low-level helpers ------------------------------------------------

    fn flush(&mut self) {
        self.out.flush().ok();
    }

    fn clear_screen(&mut self) {
        queue!(self.out, Clear(ClearType::All), cursor::MoveTo(0, 0)).ok();
    }

    fn clear_all(&mut self) {
        if self.graphics {
            graphics::clear(&mut self.out).ok();
        }
        self.clear_screen();
    }

    fn text(&mut self, col: u16, row: u16, s: &str, c: Rgb) {
        if row >= self.h || col >= self.w {
            return;
        }
        queue!(
            self.out,
            cursor::MoveTo(col, row),
            SetForegroundColor(c.crossterm()),
            Print(s)
        )
        .ok();
    }

    fn center(&mut self, row: u16, s: &str, c: Rgb) {
        let len = s.chars().count() as u16;
        let col = self.w.saturating_sub(len) / 2;
        self.text(col, row, s, c);
    }

    fn bell_ring(&mut self) {
        if self.bell {
            self.out.write_all(b"\x07").ok();
        }
    }

    /// Sleep `ms`, returning true if the beat should end early (any advance or
    /// skip key). A skip-all key also latches `skip_all` so later reveals are
    /// bypassed entirely.
    fn nap(&mut self, ms: u64) -> bool {
        if self.skip_all {
            return true;
        }
        let scale = if self.fast { 3 } else { 1 };
        let dur = Duration::from_millis(ms / scale);
        let start = Instant::now();
        while start.elapsed() < dur {
            let remain = dur - start.elapsed();
            let poll = remain.min(Duration::from_millis(25));
            if event::poll(poll).unwrap_or(false) {
                if let Ok(Event::Key(k)) = event::read() {
                    match classify(k) {
                        Input::SkipAll => {
                            self.skip_all = true;
                            return true;
                        }
                        Input::Advance => return true,
                        Input::Other => {}
                    }
                }
            }
        }
        false
    }

    /// Wait for the player to advance. Returns immediately while skipping all.
    /// Pressing a skip-all key here latches `skip_all`.
    pub fn wait_key(&mut self) {
        if self.skip_all {
            return;
        }
        loop {
            match event::read() {
                Ok(Event::Key(k)) => match classify(k) {
                    Input::SkipAll => {
                        self.skip_all = true;
                        return;
                    }
                    Input::Advance => return,
                    Input::Other => return,
                },
                Ok(_) => continue,
                // Terminal/stdin gone — don't spin forever.
                Err(_) => return,
            }
        }
    }

    fn drain_input(&mut self) {
        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            let _ = event::read();
        }
    }

    // ---- backdrops --------------------------------------------------------

    fn draw_stars(&mut self, frame: u64, dim: f32) {
        let stars = self.stars.clone();
        for (x, y, phase) in stars {
            let b = 0.45 + 0.55 * ((frame as f32 * 0.18 + phase).sin() * 0.5 + 0.5);
            let b = (b * dim).clamp(0.0, 1.0);
            let glyph = if b > 0.85 {
                "✦"
            } else if b > 0.6 {
                "✧"
            } else if b > 0.4 {
                "·"
            } else {
                " "
            };
            let c = Rgb::new(220, 226, 255).scale(b);
            self.text(x, y, glyph, c);
        }
    }

    // ---- public sequence --------------------------------------------------

    /// The shared build-up: night sky, a streaking star, and a burst that
    /// flashes the colour of the *best* pull in the batch — the anticipation.
    pub fn build_up(&mut self, best: Rarity) {
        let cx = self.w as f32 / 2.0;
        let cy = self.h as f32 / 2.0;

        // Twinkling calm before the wish.
        for f in 0..14u64 {
            self.clear_screen();
            self.draw_stars(f, 0.8);
            self.center(self.h / 2, "the night holds its breath . . .", Rgb::new(120, 130, 170));
            self.center(self.h - 1, "( esc: skip to results )", Rgb::new(80, 90, 120));
            self.flush();
            if self.nap(55) {
                break;
            }
        }

        // The comet streaks in from the upper-left toward the centre.
        let steps = 22u64;
        let mut trail: Vec<(f32, f32)> = Vec::new();
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let x = 2.0 + (cx - 2.0) * t;
            let y = 1.0 + (cy - 1.0) * t;
            trail.push((x, y));
            if trail.len() > 7 {
                trail.remove(0);
            }
            self.clear_screen();
            self.draw_stars(i, 0.5);
            for (j, (tx, ty)) in trail.iter().enumerate() {
                let f = j as f32 / trail.len() as f32;
                let glyph = if j == trail.len() - 1 { "★" } else { "·" };
                let c = Rgb::new(255, 255, 255).scale(0.4 + 0.6 * f);
                self.text(*tx as u16, *ty as u16, glyph, c);
            }
            self.flush();
            if self.nap(38) {
                break;
            }
        }

        // Impact — expanding rings in the rarity colour.
        let col = best.reveal_color();
        if best == Rarity::Five {
            self.bell_ring();
        }
        let max_r = (self.w.max(self.h) as f32) / 2.0;
        let mut r = 1.0;
        while r < max_r {
            self.clear_screen();
            self.draw_stars(steps + r as u64, 0.3);
            self.draw_ring(cx, cy, r, col, "◆");
            if r > 2.0 {
                self.draw_ring(cx, cy, r - 2.0, col.scale(0.6), "•");
            }
            self.flush();
            if self.nap(24) {
                break;
            }
            r += 2.2;
        }

        // A held flash of the colour — the moment you know what you got.
        for k in 0..6 {
            self.clear_screen();
            let intensity = 1.0 - k as f32 / 6.0;
            self.flood(col.scale(0.35 + intensity * 0.65));
            let label = match best {
                Rarity::Five => "✦  ✦  ✦",
                Rarity::Four => "✧  ✧  ✧",
                Rarity::Three => "·  ·  ·",
            };
            self.center(self.h / 2, label, Rgb::new(20, 20, 30));
            self.flush();
            if self.nap(60) {
                break;
            }
        }
        self.nap(120);
    }

    fn draw_ring(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, glyph: &str) {
        let n = ((r * 6.0) as usize).max(8);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            // Compensate for cells being ~twice as tall as wide.
            let x = cx + a.cos() * r * 1.9;
            let y = cy + a.sin() * r;
            if x >= 0.0 && y >= 0.0 && (x as u16) < self.w && (y as u16) < self.h {
                self.text(x as u16, y as u16, glyph, c);
            }
        }
    }

    fn flood(&mut self, c: Rgb) {
        // Sparse block fill so the flash reads without hammering the terminal.
        let row_line: String = "█".repeat(self.w as usize);
        for y in 0..self.h {
            if y % 1 == 0 {
                self.text(0, y, &row_line, c);
            }
        }
    }

    // ---- item reveal ------------------------------------------------------

    /// Reveal one item's card with name, element and igniting stars.
    /// 5-stars are routed through the grand cutscene instead.
    pub fn reveal(&mut self, item: &Item, outcome: &WishOutcome, is_new: bool, count: u32) {
        // Player asked to skip to results — don't draw this item at all.
        if self.skip_all {
            return;
        }
        if item.rarity == Rarity::Five {
            self.cutscene(item, outcome, is_new, count);
            return;
        }
        self.clear_all();
        self.draw_stars(0, 0.4);

        let png = art::card_png(item);
        let img_col = self.w.saturating_sub(self.img_cols) / 2;
        if self.graphics {
            queue!(self.out, cursor::MoveTo(img_col, 1)).ok();
            self.flush();
            graphics::draw_png(&mut self.out, &png, self.img_cols, self.img_rows).ok();
        } else {
            self.ascii_placeholder(img_col, 1, item);
        }

        let base = self.img_rows + 2;
        self.reveal_labels(item, outcome, is_new, count, base);
        self.center(self.h - 1, "( space: next   ·   esc: skip to results )", Rgb::new(110, 120, 150));
        self.flush();
        self.drain_input();
        self.wait_key();
    }

    fn reveal_labels(
        &mut self,
        item: &Item,
        outcome: &WishOutcome,
        is_new: bool,
        count: u32,
        base: u16,
    ) {
        let rc = item.rarity.reveal_color();
        // Stars ignite one by one.
        let mut star_line = String::new();
        for i in 0..item.rarity.stars() {
            star_line.push('★');
            self.center(base, &format!("{:<width$}", star_line, width = item.rarity.stars()), rc);
            self.flush();
            if i + 1 == item.rarity.stars() && item.rarity >= Rarity::Four {
                self.bell_ring();
            }
            self.nap(140);
        }

        // Name typed out.
        let name = item.name;
        for i in 1..=name.chars().count() {
            let partial: String = name.chars().take(i).collect();
            self.center(base + 2, &partial, Rgb::new(245, 245, 255));
            self.flush();
            self.nap(28);
        }

        let sub = if item.is_character() {
            let el = item.element().unwrap();
            format!("{}  {}  ·  {}", el.glyph(), el.name(), item.weapon_type().name())
        } else {
            format!("Weapon  ·  {}", item.weapon_type().name())
        };
        self.center(base + 3, &sub, item.theme.accent);
        self.center(base + 4, item.title, Rgb::new(170, 175, 200));

        let tag = if is_new {
            "NEW!"
        } else if item.is_character() {
            "Constellation +1"
        } else {
            "Refinement +1"
        };
        let tag_col = if is_new { Rgb::new(255, 220, 120) } else { Rgb::new(150, 200, 160) };
        let extra = if outcome.featured && outcome.rarity >= Rarity::Four {
            format!("{}   ·  rate-up", tag)
        } else {
            format!("{}   ·  copy #{}", tag, count)
        };
        self.center(base + 5, &extra, tag_col);
    }

    /// The grand 5-star cutscene. Every 5-star earns this.
    fn cutscene(&mut self, item: &Item, outcome: &WishOutcome, is_new: bool, count: u32) {
        if self.skip_all {
            return;
        }
        let gold = Rgb::new(255, 208, 92);
        self.clear_all();

        // 1. Gold dust swirls up from darkness.
        for f in 0..18u64 {
            self.clear_screen();
            self.draw_gold_dust(f);
            let title = "✦   A   N E W   S T A R   D E S C E N D S   ✦";
            let fade = (f as f32 / 10.0).min(1.0);
            self.center(self.h / 2, title, gold.scale(fade));
            self.flush();
            if self.nap(60) {
                break;
            }
        }
        self.nap(180);

        // 2. Radiant rings bloom outward, then the portrait rises in.
        self.clear_all();
        let cx = self.w as f32 / 2.0;
        let cy = self.h as f32 / 2.0;
        for i in 0..10 {
            self.clear_screen();
            self.draw_gold_dust(i);
            self.draw_ring(cx, cy, 2.0 + i as f32 * 1.6, gold, "◇");
            self.flush();
            if self.nap(45) {
                break;
            }
        }
        self.bell_ring();

        // 3. The portrait.
        self.clear_all();
        self.draw_gold_dust(0);
        let png = art::card_png(item);
        let img_col = self.w.saturating_sub(self.img_cols) / 2;
        if self.graphics {
            queue!(self.out, cursor::MoveTo(img_col, 1)).ok();
            self.flush();
            graphics::draw_png(&mut self.out, &png, self.img_cols, self.img_rows).ok();
        } else {
            self.ascii_placeholder(img_col, 1, item);
        }

        let base = self.img_rows + 2;

        // Five stars ignite with a beat and a bell each.
        let mut line = String::new();
        for _ in 0..5 {
            line.push('★');
            self.center(base, &line, gold);
            self.flush();
            self.bell_ring();
            self.nap(180);
        }

        // Name, big and gold, typed out.
        let name = item.name.to_uppercase();
        for i in 1..=name.chars().count() {
            let partial: String = name.chars().take(i).collect();
            self.center(base + 2, &partial, Rgb::new(255, 240, 200));
            self.flush();
            self.nap(45);
        }

        // Fill in the remaining captions over the static gold dust and hold.
        // We deliberately do NOT clear the screen again — on some terminals a
        // screen-erase drops the inline image, so once the portrait is placed
        // we only ever paint text on top of it.
        self.draw_cutscene_text(item, outcome, is_new, count, base, gold);
        self.center(self.h - 1, "( space: next   ·   esc: skip to results )", Rgb::new(150, 140, 110));
        self.flush();

        self.drain_input();
        self.wait_key();
    }

    /// Paint the full 5-star label block (stars, name, element, title, status,
    /// tag, flavor). Called every shimmer frame so dust never covers the text.
    fn draw_cutscene_text(
        &mut self,
        item: &Item,
        outcome: &WishOutcome,
        is_new: bool,
        count: u32,
        base: u16,
        gold: Rgb,
    ) {
        self.center(base, "★★★★★", gold);
        self.center(base + 2, &item.name.to_uppercase(), Rgb::new(255, 240, 200));

        let el = item
            .element()
            .map(|e| format!("{}  {}  ·  {}", e.glyph(), e.name(), item.weapon_type().name()))
            .unwrap_or_else(|| format!("Weapon  ·  {}", item.weapon_type().name()));
        self.center(base + 3, &el, item.theme.accent);
        self.center(base + 4, item.title, gold.scale(0.85));

        let status = if outcome.radiance {
            "✦ CAPTURING RADIANCE — the featured star answers your call ✦".to_string()
        } else if outcome.featured {
            "★ rate-up WON ★".to_string()
        } else if outcome.lost_5050 {
            "( lost the 50/50 — next 5-star is guaranteed featured )".to_string()
        } else {
            String::new()
        };
        if !status.is_empty() {
            let sc = if outcome.featured || outcome.radiance {
                Rgb::new(255, 225, 140)
            } else {
                Rgb::new(180, 160, 210)
            };
            self.center(base + 5, &status, sc);
        }

        let tag = if is_new {
            "NEW CHARACTER".to_string()
        } else {
            format!("Constellation +1  ·  copy #{count}")
        };
        self.center(base + 6, &tag, Rgb::new(255, 235, 190));
        self.center(base + 7, item.flavor, Rgb::new(200, 195, 210));
    }

    fn draw_gold_dust(&mut self, frame: u64) {
        let stars = self.stars.clone();
        for (x, _y, phase) in stars {
            let rise = (frame as f32 * 0.5 + phase * 3.0) % self.h as f32;
            let yy = (self.h as f32 - rise) as u16 % self.h;
            let b = 0.4 + 0.6 * ((frame as f32 * 0.2 + phase).sin() * 0.5 + 0.5);
            let glyph = if b > 0.8 { "✦" } else if b > 0.55 { "✧" } else { "·" };
            let c = Rgb::new(255, 210, 120).scale(b);
            self.text(x, yy, glyph, c);
        }
    }

    fn ascii_placeholder(&mut self, col: u16, row: u16, item: &Item) {
        // Shown only when the terminal has no image support.
        let c = item.theme.accent;
        let box_w = self.img_cols;
        let rows = self.img_rows;
        let border: String = "─".repeat(box_w as usize - 2);
        self.text(col, row, &format!("┌{}┐", border), c);
        for r in 1..rows - 1 {
            self.text(col, row + r, &format!("│{}│", " ".repeat(box_w as usize - 2)), c.scale(0.6));
        }
        self.text(col, row + rows - 1, &format!("└{}┘", border), c);
        self.center(row + rows / 2, "[ image needs Ghostty ]", c);
    }

    /// A closing summary of the batch.
    pub fn summary(&mut self, items: &[(&Item, bool)]) {
        // Land here after a skip and stay until the player is ready.
        self.skip_all = false;
        self.clear_all();
        self.draw_stars(0, 0.5);
        self.center(1, "— wish results —", Rgb::new(220, 210, 180));
        let start = 3u16;
        for (i, (item, is_new)) in items.iter().enumerate() {
            let row = start + i as u16;
            if row >= self.h - 2 {
                break;
            }
            let rc = item.rarity.reveal_color();
            let stars: String = "★".repeat(item.rarity.stars());
            let newtag = if *is_new { "  NEW" } else { "" };
            let line = format!("{:<5} {:<20} {}{}", stars, item.name, item.kind_label(), newtag);
            self.text(4, row, &line, rc);
        }
        self.center(self.h - 1, "( press space to return )", Rgb::new(110, 120, 150));
        self.flush();
        self.drain_input();
        self.wait_key();
    }

    /// Draw `s` centred within a `width`-cell span starting at column `x`,
    /// truncating if it doesn't fit.
    fn text_span(&mut self, x: u16, width: u16, row: u16, s: &str, c: Rgb) {
        let shown: String = s.chars().take(width as usize).collect();
        let len = shown.chars().count() as u16;
        let off = (width.saturating_sub(len)) / 2;
        self.text(x + off, row, &shown, c);
    }

    /// The collection page: a grid of pixel-art character busts (owned show
    /// their art; unowned are locked), with an owned-weapons footer.
    /// Block until a key; returns its code (no skip semantics).
    fn read_key(&mut self) -> KeyCode {
        loop {
            match event::read() {
                Ok(Event::Key(k)) => return k.code,
                Ok(_) => continue,
                Err(_) => return KeyCode::Esc,
            }
        }
    }

    /// The interactive collection: a grid of pixel-art busts; press a number to
    /// view that character large (arcade select-screen style).
    pub fn gallery(&mut self, chars: &[(&Item, u32)], weapons: &[(&Item, u32)]) {
        self.skip_all = false;
        loop {
            self.draw_gallery_grid(chars, weapons);
            match self.read_key() {
                KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                    let idx = c.to_digit(10).unwrap() as usize;
                    if idx <= chars.len() {
                        let (item, count) = chars[idx - 1];
                        if count > 0 {
                            self.show_character(item, count);
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char(' ') | KeyCode::Enter => break,
                _ => {}
            }
        }
    }

    fn draw_gallery_grid(&mut self, chars: &[(&Item, u32)], weapons: &[(&Item, u32)]) {
        self.clear_all();
        let gold = Rgb::new(255, 208, 92);
        self.center(0, "✦   C O L L E C T I O N   ✦", gold);

        // Square-ish icon cells (the sprites are square).
        let ic_w = 12u16;
        let ic_h = 6u16;
        let block_w = ic_w + 3;
        let block_h = ic_h + 3;
        let cols = (self.w / block_w).clamp(1, chars.len().max(1) as u16);
        let left = self.w.saturating_sub(cols * block_w) / 2 + 1;
        let top = 2u16;

        for (i, (item, count)) in chars.iter().enumerate() {
            let cxi = i as u16 % cols;
            let cyi = i as u16 / cols;
            let x = left + cxi * block_w;
            let y = top + cyi * block_h;
            if y + ic_h + 2 >= self.h {
                break;
            }
            let owned = *count > 0;
            let rc = item.rarity.reveal_color();

            if owned && self.graphics {
                let png = art::pixel_png(item);
                queue!(self.out, cursor::MoveTo(x, y)).ok();
                self.flush();
                graphics::draw_png(&mut self.out, &png, ic_w, ic_h).ok();
            } else {
                let col = if owned { rc.scale(0.7) } else { Rgb::new(70, 74, 90) };
                let border = "─".repeat(ic_w as usize - 2);
                self.text(x, y, &format!("┌{}┐", border), col);
                for r in 1..ic_h - 1 {
                    self.text(x, y + r, &format!("│{}│", " ".repeat(ic_w as usize - 2)), col);
                }
                self.text(x, y + ic_h - 1, &format!("└{}┘", border), col);
                let mark = if owned { "★".to_string() } else { "?".to_string() };
                self.text_span(x, ic_w, y + ic_h / 2, &mark, col);
            }

            let name = if owned {
                format!("{} {}", i + 1, item.name)
            } else {
                format!("{} ???", i + 1)
            };
            let name_col = if owned { Rgb::new(235, 235, 245) } else { Rgb::new(92, 96, 112) };
            self.text_span(x, ic_w, y + ic_h, &name, name_col);

            let stars: String = "★".repeat(item.rarity.stars());
            let info = if owned {
                format!("{} C{}", stars, count.saturating_sub(1))
            } else {
                stars
            };
            let info_col = if owned { rc } else { Rgb::new(80, 84, 100) };
            self.text_span(x, ic_w, y + ic_h + 1, &info, info_col);
        }

        // Owned-weapons footer.
        let rows_used = (chars.len() as u16 + cols - 1) / cols;
        let wy = top + rows_used * block_h;
        if wy + 1 < self.h - 1 {
            self.text(2, wy, "Weapons", gold.scale(0.9));
            let names: Vec<String> = weapons
                .iter()
                .filter(|(_, c)| *c > 0)
                .map(|(it, c)| {
                    if *c > 1 {
                        format!("{} R{}", it.name, (*c).min(5))
                    } else {
                        it.name.to_string()
                    }
                })
                .collect();
            let line = if names.is_empty() {
                "— none yet —".to_string()
            } else {
                names.join("   ")
            };
            let shown: String = line.chars().take(self.w.saturating_sub(4) as usize).collect();
            self.text(2, wy + 1, &shown, Rgb::new(150, 160, 180));
        }

        self.center(self.h - 1, "( number: view  ·  space: back )", Rgb::new(110, 120, 150));
        self.flush();
        self.drain_input();
    }

    /// Large single-character view — the arcade select-screen close-up.
    fn show_character(&mut self, item: &Item, count: u32) {
        self.clear_all();
        let rc = item.rarity.reveal_color();

        // A large square-ish image box (the sprites are square).
        let rows = (self.h.saturating_sub(8)).clamp(8, 22);
        let cols = (rows * 2).min(self.w.saturating_sub(2));
        let col = self.w.saturating_sub(cols) / 2;
        if self.graphics {
            let png = art::pixel_png(item);
            queue!(self.out, cursor::MoveTo(col, 1)).ok();
            self.flush();
            graphics::draw_png(&mut self.out, &png, cols, rows).ok();
        } else {
            self.ascii_placeholder(col, 1, item);
        }

        let base = rows + 2;
        self.center(base, &format!("{} {}", "★".repeat(item.rarity.stars()), item.name), rc);
        let el = item
            .element()
            .map(|e| format!("{}  {}  ·  {}", e.glyph(), e.name(), item.weapon_type().name()))
            .unwrap_or_else(|| format!("Weapon  ·  {}", item.weapon_type().name()));
        self.center(base + 1, &el, item.theme.accent);
        self.center(base + 2, item.title, Rgb::new(180, 185, 205));
        self.center(base + 3, &format!("Constellation {}  ·  {} copies", count.saturating_sub(1), count), Rgb::new(200, 200, 215));
        self.center(base + 5, item.flavor, Rgb::new(170, 170, 190));

        self.center(self.h - 1, "( press space to go back )", Rgb::new(110, 120, 150));
        self.flush();
        self.drain_input();
        self.read_key();
    }
}

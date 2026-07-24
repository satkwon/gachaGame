//! A small widget toolkit: a persistent full-screen session with a
//! keyboard-navigable button menu and a message box. Replaces the old
//! typed-number, line-based menus.

use std::io::{stdout, Stdout, Write};

use crossterm::event::{self, Event, KeyCode};
use crossterm::style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};

use crate::model::Rgb;
use crate::{art, graphics};

const GOLD: Rgb = Rgb::new(255, 208, 92);
const LIGHT: Rgb = Rgb::new(232, 234, 244);
const GREY: Rgb = Rgb::new(150, 158, 182);
const DARK: Rgb = Rgb::new(18, 18, 26);
/// Solid panel the menu text sits on, so it stays readable over the backdrop.
const PANEL: Rgb = Rgb::new(16, 18, 32);
/// Characters decorating the main menu (left and right — the centred panel
/// would cover anything in the middle).
const FEATURED: [&str; 2] = ["menu_guren", "menu_yukihana"];

/// One selectable button, with optional detail lines shown when highlighted.
pub struct MenuItem {
    pub label: String,
    pub detail: Vec<String>,
    pub enabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        MenuItem { label: label.into(), detail: Vec::new(), enabled: true }
    }
    pub fn detail(mut self, lines: Vec<String>) -> Self {
        self.detail = lines;
        self
    }
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Visible width of a string, ignoring ANSI escape sequences.
pub fn viz_len(s: &str) -> usize {
    let mut n = 0usize;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for nc in chars.by_ref() {
                if nc.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            n += 1;
        }
    }
    n
}

pub struct Ui {
    out: Stdout,
}

impl Ui {
    pub fn new() -> Self {
        let mut out = stdout();
        terminal::enable_raw_mode().ok();
        execute!(out, terminal::EnterAlternateScreen, cursor::Hide).ok();
        Ui { out }
    }

    pub fn close(&mut self) {
        execute!(self.out, cursor::Show, terminal::LeaveAlternateScreen).ok();
        terminal::disable_raw_mode().ok();
    }

    pub fn size(&self) -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }

    fn at(&mut self, col: u16, row: u16, s: &str, fg: Rgb) {
        queue!(
            self.out,
            cursor::MoveTo(col, row),
            SetForegroundColor(fg.crossterm()),
            Print(s),
            ResetColor
        )
        .ok();
    }

    fn center(&mut self, row: u16, s: &str, fg: Rgb) {
        let (w, _) = self.size();
        let col = w.saturating_sub(viz_len(s) as u16) / 2;
        self.at(col, row, s, fg);
    }

    /// Fill a solid rectangle (the panel the menu sits on).
    fn panel(&mut self, col: u16, row: u16, width: u16, height: u16, bg: Rgb) {
        let blank = " ".repeat(width as usize);
        for r in 0..height {
            queue!(
                self.out,
                cursor::MoveTo(col, row + r),
                SetBackgroundColor(bg.crossterm()),
                Print(&blank),
                ResetColor
            )
            .ok();
        }
    }

    /// Text drawn on the panel background.
    fn at_bg(&mut self, col: u16, row: u16, s: &str, fg: Rgb) {
        queue!(
            self.out,
            cursor::MoveTo(col, row),
            SetBackgroundColor(PANEL.crossterm()),
            SetForegroundColor(fg.crossterm()),
            Print(s),
            ResetColor
        )
        .ok();
    }

    fn center_bg(&mut self, row: u16, s: &str, fg: Rgb) {
        let (w, _) = self.size();
        let col = w.saturating_sub(viz_len(s) as u16) / 2;
        self.at_bg(col, row, s, fg);
    }

    /// Draw the decorated backdrop behind everything (negative z).
    fn draw_backdrop(&mut self) {
        if !graphics::supported() {
            return;
        }
        let (w, h) = self.size();
        let png = art::menu_backdrop(&FEATURED);
        queue!(self.out, cursor::MoveTo(0, 0)).ok();
        self.out.flush().ok();
        graphics::draw_png_frame_z(&mut self.out, &png, w, h, 900, -2).ok();
    }

    fn button(&mut self, row: u16, label: &str, width: u16, selected: bool, enabled: bool) {
        let (w, _) = self.size();
        let col = w.saturating_sub(width) / 2;
        let pad = (width as usize).saturating_sub(viz_len(label)) / 2;
        let text = format!(
            "{}{}{}",
            " ".repeat(pad),
            label,
            " ".repeat((width as usize).saturating_sub(pad + viz_len(label)))
        );
        // Solid background in every state → reads as a real button.
        let (bg, fg) = if selected {
            (GOLD, DARK)
        } else if enabled {
            (Rgb::new(42, 48, 72), LIGHT)
        } else {
            (Rgb::new(26, 28, 40), Rgb::new(104, 108, 126))
        };
        queue!(
            self.out,
            cursor::MoveTo(col, row),
            SetBackgroundColor(bg.crossterm()),
            SetForegroundColor(fg.crossterm()),
            Print(text),
            ResetColor
        )
        .ok();
    }

    /// Render a titled button menu and drive it until the player picks or
    /// cancels. Returns the chosen index, or None on cancel (Esc) when allowed.
    pub fn menu(
        &mut self,
        title: &str,
        header: &[String],
        items: &[MenuItem],
        cancelable: bool,
    ) -> Option<usize> {
        let mut sel = 0usize;
        let width = items
            .iter()
            .map(|i| viz_len(&i.label))
            .max()
            .unwrap_or(10)
            .clamp(18, 60) as u16
            + 6;

        // Layout is fixed for this menu, so the backdrop + panel are painted
        // once and navigation only repaints the buttons/detail (no image
        // re-transmit → responsive and flicker-free).
        let (_, h) = self.size();
        let detail_max = items.iter().map(|i| i.detail.len()).max().unwrap_or(0) as u16;
        let hdr_h = if header.is_empty() { 0 } else { header.len() as u16 + 1 };
        let det_h = if detail_max > 0 { detail_max + 1 } else { 0 };
        let total = 2 + hdr_h + items.len() as u16 + 1 + det_h + 1;
        let top = h.saturating_sub(total) / 2;
        let btn_start = top + 2 + hdr_h;
        let det_start = btn_start + items.len() as u16 + 1;
        self.render_menu_full(title, header, items, sel, width, top, total, btn_start, det_start, det_h);

        loop {
            match event::read() {
                Ok(Event::Key(k)) => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        sel = if sel == 0 { items.len() - 1 } else { sel - 1 };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        sel = (sel + 1) % items.len();
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if items[sel].enabled {
                            return Some(sel);
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        if cancelable {
                            return None;
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                        let i = c as usize - '1' as usize;
                        if i < items.len() && items[i].enabled {
                            return Some(i);
                        }
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(_) => return None,
            }
            self.render_menu_dynamic(items, sel, width, btn_start, det_start, det_h);
        }
    }

    /// Full paint: backdrop image, solid panel, then all the text.
    #[allow(clippy::too_many_arguments)]
    fn render_menu_full(
        &mut self,
        title: &str,
        header: &[String],
        items: &[MenuItem],
        sel: usize,
        width: u16,
        top: u16,
        total: u16,
        btn_start: u16,
        det_start: u16,
        det_h: u16,
    ) {
        let (w, h) = self.size();
        queue!(self.out, Clear(ClearType::All)).ok();
        self.draw_backdrop();

        // Panel wide enough for the widest element.
        let widest = header
            .iter()
            .chain(items.iter().flat_map(|i| i.detail.iter()))
            .map(|s| viz_len(s) as u16)
            .max()
            .unwrap_or(0)
            .max(width)
            .max(viz_len(title) as u16);
        let pw = (widest + 8).min(w.saturating_sub(2));
        let pcol = w.saturating_sub(pw) / 2;
        self.panel(pcol, top.saturating_sub(1), pw, total + 2, PANEL);

        self.center_bg(top, title, GOLD);
        let mut row = top + 2;
        for line in header {
            self.center_bg(row, line, GREY);
            row += 1;
        }
        for (i, item) in items.iter().enumerate() {
            self.button(btn_start + i as u16, &item.label, width, i == sel, item.enabled);
        }
        let _ = det_start;
        let _ = det_h;
        self.render_menu_dynamic(items, sel, width, btn_start, det_start, det_h);
        self.center(h.saturating_sub(1), "↑↓ move   ·   enter select   ·   esc back", GREY);
        self.out.flush().ok();
    }

    /// Repaint just the buttons and the selected item's detail lines.
    fn render_menu_dynamic(
        &mut self,
        items: &[MenuItem],
        sel: usize,
        width: u16,
        btn_start: u16,
        det_start: u16,
        det_h: u16,
    ) {
        for (i, item) in items.iter().enumerate() {
            self.button(btn_start + i as u16, &item.label, width, i == sel, item.enabled);
        }
        if det_h > 0 {
            let (w, _) = self.size();
            // Clear the detail block on the panel, then draw the current detail.
            let pw = w.saturating_sub(4);
            self.panel(2, det_start, pw, det_h, PANEL);
            for (k, line) in items[sel].detail.iter().enumerate() {
                self.center_bg(det_start + k as u16, line, Rgb::new(186, 192, 212));
            }
        }
        self.out.flush().ok();
    }

    /// Pick up to `max` items (space toggles, enter confirms). Returns the
    /// chosen indices in selection order, or empty on cancel.
    pub fn multi_select(&mut self, title: &str, header: &[String], items: &[MenuItem], max: usize) -> Vec<usize> {
        let mut sel = 0usize;
        let mut chosen: Vec<usize> = Vec::new();
        let width = items
            .iter()
            .map(|i| viz_len(&i.label))
            .max()
            .unwrap_or(10)
            .clamp(18, 56) as u16
            + 10;

        loop {
            let (_, h) = self.size();
            let hdr_h = if header.is_empty() { 0 } else { header.len() as u16 + 1 };
            let total = 2 + hdr_h + items.len() as u16 + 2;
            let mut row = h.saturating_sub(total) / 2;
            queue!(self.out, Clear(ClearType::All)).ok();
            self.center(row, title, GOLD);
            row += 2;
            for line in header {
                self.center(row, line, GREY);
                row += 1;
            }
            if hdr_h > 0 {
                row += 1;
            }
            for (i, item) in items.iter().enumerate() {
                let mark = if chosen.contains(&i) { "✓ " } else { "  " };
                self.button(row, &format!("{}{}", mark, item.label), width, i == sel, item.enabled);
                row += 1;
            }
            let footer = format!(
                "{}/{} selected   ·   space toggle   ·   enter confirm   ·   esc cancel",
                chosen.len(),
                max
            );
            self.center(h.saturating_sub(1), &footer, GREY);
            self.out.flush().ok();

            let toggle = |chosen: &mut Vec<usize>, i: usize| {
                if let Some(pos) = chosen.iter().position(|&c| c == i) {
                    chosen.remove(pos);
                } else if chosen.len() < max && items[i].enabled {
                    chosen.push(i);
                }
            };
            match event::read() {
                Ok(Event::Key(k)) => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => sel = if sel == 0 { items.len() - 1 } else { sel - 1 },
                    KeyCode::Down | KeyCode::Char('j') => sel = (sel + 1) % items.len(),
                    KeyCode::Char(' ') => toggle(&mut chosen, sel),
                    KeyCode::Enter => {
                        if !chosen.is_empty() {
                            return chosen;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => return Vec::new(),
                    KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                        let i = c as usize - '1' as usize;
                        if i < items.len() {
                            toggle(&mut chosen, i);
                        }
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(_) => return Vec::new(),
            }
        }
    }

    /// Show a centered block of text and wait for any key.
    pub fn message(&mut self, lines: &[String], title: &str) {
        let (_, h) = self.size();
        let total = 2 + lines.len() as u16 + 1;
        let mut row = h.saturating_sub(total) / 2;
        queue!(self.out, Clear(ClearType::All)).ok();
        if !title.is_empty() {
            self.center(row, title, GOLD);
        }
        row += 2;
        for line in lines {
            self.center(row, line, LIGHT);
            row += 1;
        }
        self.center(h.saturating_sub(1), "( press enter )", GREY);
        self.out.flush().ok();
        self.wait_key();
    }

    /// A Yes/No confirmation (defaults to No).
    pub fn confirm(&mut self, title: &str, warning: &[String]) -> bool {
        let items = [MenuItem::new("No"), MenuItem::new("Yes")];
        matches!(self.menu(title, warning, &items, true), Some(1))
    }

    pub fn wait_key(&mut self) {
        loop {
            match event::read() {
                Ok(Event::Key(_)) => return,
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }
}

//! A small widget toolkit: a persistent full-screen session with a
//! keyboard-navigable button menu and a message box. Replaces the old
//! typed-number, line-based menus.

use std::io::{stdout, Stdout, Write};

use crossterm::event::{self, Event, KeyCode};
use crossterm::style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{cursor, execute, queue};

use crate::model::Rgb;

const GOLD: Rgb = Rgb::new(255, 208, 92);
const LIGHT: Rgb = Rgb::new(232, 234, 244);
const GREY: Rgb = Rgb::new(130, 140, 165);
const DARK: Rgb = Rgb::new(18, 18, 26);

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

    fn button(&mut self, row: u16, label: &str, width: u16, selected: bool, enabled: bool) {
        let (w, _) = self.size();
        let col = w.saturating_sub(width) / 2;
        // Center the label inside the fixed button width.
        let pad = (width as usize).saturating_sub(viz_len(label)) / 2;
        let text = format!(
            "{}{}{}",
            " ".repeat(pad),
            label,
            " ".repeat((width as usize).saturating_sub(pad + viz_len(label)))
        );
        queue!(self.out, cursor::MoveTo(col, row)).ok();
        if selected {
            queue!(
                self.out,
                SetBackgroundColor(GOLD.crossterm()),
                SetForegroundColor(DARK.crossterm()),
                Print(text),
                ResetColor
            )
            .ok();
        } else {
            let fg = if enabled { LIGHT } else { GREY };
            queue!(self.out, SetForegroundColor(fg.crossterm()), Print(text), ResetColor).ok();
        }
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

        loop {
            self.render_menu(title, header, items, sel, width);
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
        }
    }

    fn render_menu(&mut self, title: &str, header: &[String], items: &[MenuItem], sel: usize, width: u16) {
        let (_, h) = self.size();
        let detail = &items[sel].detail;
        let hdr_h = if header.is_empty() { 0 } else { header.len() as u16 + 1 };
        let det_h = if detail.is_empty() { 0 } else { detail.len() as u16 + 1 };
        let total = 2 + hdr_h + items.len() as u16 + 1 + det_h + 1; // title,blank,hdr,items,blank,detail,footer
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
            self.button(row, &item.label, width, i == sel, item.enabled);
            row += 1;
        }
        row += 1;
        for line in detail {
            self.center(row, line, Rgb::new(180, 186, 205));
            row += 1;
        }
        let footer = "↑↓ move   ·   enter select   ·   esc back";
        self.center(h.saturating_sub(1), footer, GREY);
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

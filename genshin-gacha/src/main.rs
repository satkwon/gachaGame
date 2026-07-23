//! A terminal gacha / wish simulator with Genshin-accurate odds and Ghostty
//! inline-image reveals.

mod anim;
mod art;
mod data;
mod gacha;
mod graphics;
mod model;
mod save;

use std::io::{stdout, Write};

use crossterm::terminal;
use crossterm::{cursor, execute};
use rand::thread_rng;

use anim::Stage;
use gacha::{PityState, WishOutcome};
use model::{Item, Rarity};
use save::SaveData;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const GOLD: &str = "\x1b[38;2;255;208;92m";
const PURPLE: &str = "\x1b[38;2;196;120;255m";
const CYAN: &str = "\x1b[38;2;120;200;230m";
const GREY: &str = "\x1b[38;2;140;150;175m";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("--simulate") => {
            let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let banner = args.get(2).map(|s| s.as_str()).unwrap_or("snowfall");
            simulate(n, banner);
            return;
        }
        Some("--export-prompts") => {
            let items: Vec<serde_json::Value> = data::ROSTER
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "id": i.id,
                        "name": i.name,
                        "kind": i.kind_label().to_lowercase(),
                        "element": i.element().map(|e| e.name()),
                        "weapon": i.weapon_type().name(),
                        "rarity": i.rarity.stars(),
                        "look": i.look,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&items).unwrap());
            return;
        }
        Some("--dump-pixels") => {
            let dir = args.get(1).map(|s| s.as_str()).unwrap_or("pixel-preview");
            std::fs::create_dir_all(dir).ok();
            for item in data::ROSTER.iter().filter(|i| i.is_character()) {
                let png = art::pixel_png(item);
                let path = format!("{dir}/{}.png", item.id);
                std::fs::write(&path, &png).ok();
                println!("wrote {path} ({} bytes)", png.len());
            }
            return;
        }
        Some("--dump-art") => {
            let dir = args.get(1).map(|s| s.as_str()).unwrap_or("art-preview");
            std::fs::create_dir_all(dir).ok();
            for item in data::ROSTER {
                let png = art::card_png(item);
                let path = format!("{dir}/{}.png", item.id);
                std::fs::write(&path, &png).ok();
                println!("wrote {path} ({} bytes)", png.len());
            }
            return;
        }
        Some("--help" | "-h") => {
            print_help();
            return;
        }
        _ => {}
    }

    // Restore the terminal even if a panic unwinds through raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        default_hook(info);
    }));

    interactive();
}

// ---------------------------------------------------------------------------
// Interactive menu loop
// ---------------------------------------------------------------------------

fn interactive() {
    let mut save = SaveData::load();
    loop {
        match screen(&home_body(&save), "› ").trim() {
            "1" => wish_menu(&mut save),
            "2" => show_collection(&save),
            "3" => show_details(&save),
            "4" => show_history(&save),
            "5" => settings_menu(&mut save),
            "6" => {
                save.primogems += 16000;
                save.intertwined_fates += 10;
                save.save();
                screen(
                    &format!("{GOLD}{BOLD}+16000 primogems, +10 fates.{RESET}"),
                    &format!("{GREY}( press enter ){RESET}"),
                );
            }
            "7" => reset_menu(&mut save),
            "0" | "q" | "quit" | "exit" | "\u{4}" => {
                print!("\x1b[2J\x1b[H\n  {GREY}Until the stars align again.{RESET}\n");
                stdout().flush().ok();
                break;
            }
            _ => {}
        }
    }
}

fn home_body(save: &SaveData) -> String {
    let p = &save.pity;
    let guar5 = if p.guaranteed_5 { "GUARANTEED" } else { "50/50" };
    let bar = "✦ · ─────────────────────────────────────────── · ✦";
    let mut s = String::new();
    s += &format!("{GOLD}{BOLD}{bar}{RESET}\n");
    s += &format!("{GOLD}{BOLD}W  I  S  H     S  I  M  U  L  A  T  O  R{RESET}\n");
    s += &format!("{GOLD}{BOLD}{bar}{RESET}\n\n");
    s += &format!(
        "{CYAN}Intertwined Fates{RESET} {BOLD}{}{RESET}    {PURPLE}Primogems{RESET} {}    {GREY}(≈ {} wishes){RESET}\n",
        save.intertwined_fates, save.primogems, save.available_wishes()
    );
    s += &format!(
        "{GREY}5★ pity{RESET} {}/90   {GREY}4★ pity{RESET} {}/10   {GREY}next 5★{RESET} {}\n\n",
        p.pulls_since_5, p.pulls_since_4, guar5
    );
    s += &format!("{BOLD}1{RESET}  Wish            {BOLD}2{RESET}  Collection      {BOLD}3{RESET}  Wish details\n");
    s += &format!("{BOLD}4{RESET}  History         {BOLD}5{RESET}  Settings        {BOLD}6{RESET}  Top up (sim)\n");
    s += &format!("{BOLD}7{RESET}  Reset           {BOLD}0{RESET}  Quit");
    if !graphics::supported() {
        s += &format!("\n\n{GREY}(No Kitty graphics — run in Ghostty for image reveals){RESET}");
    }
    s
}

fn wish_menu(save: &mut SaveData) {
    loop {
        let mut body = format!("{GOLD}{BOLD}── CHOOSE A BANNER ──{RESET}\n\n");
        for (i, b) in data::BANNERS.iter().enumerate() {
            let five = data::item(b.featured_5);
            let el = five.element().map(|e| e.name()).unwrap_or("");
            body += &format!("{BOLD}{}{RESET}  {GOLD}{}{RESET}\n", i + 1, b.name);
            body += &format!(
                "{GOLD}★★★★★{RESET} {} {GREY}·{RESET} {} {GREY}{}{RESET}\n",
                five.name, el, five.title
            );
            let f4: Vec<&str> = b.featured_4.iter().map(|id| data::item(id).name).collect();
            body += &format!("{PURPLE}★★★★{RESET}  {}\n\n", f4.join(", "));
        }
        body += &format!("{GREY}0  back{RESET}");

        let choice = screen(&body, "banner › ");
        if choice == "\u{4}" {
            return;
        }
        let idx: usize = match choice.trim().parse::<usize>() {
            Ok(0) => return,
            Ok(n) if n >= 1 && n <= data::BANNERS.len() => n - 1,
            _ => continue,
        };
        count_menu(save, data::BANNERS[idx].id);
    }
}

fn count_menu(save: &mut SaveData, banner_id: &str) {
    loop {
        let b = data::banner(banner_id);
        let mut body = format!("{GOLD}{BOLD}{}{RESET}\n\n", b.name);
        body += &format!("{GREY}You have ≈ {} wishes.{RESET}\n\n", save.available_wishes());
        body += &format!("{BOLD}1{RESET}  Wish ×1     {GREY}(1 fate){RESET}\n");
        body += &format!("{BOLD}2{RESET}  Wish ×10    {GREY}(10 fates){RESET}\n");
        body += &format!("{BOLD}0{RESET}  back");
        match screen(&body, "› ").trim() {
            "1" => {
                do_wish(save, banner_id, 1);
                return;
            }
            "2" => {
                do_wish(save, banner_id, 10);
                return;
            }
            "0" | "\u{4}" => return,
            _ => {}
        }
    }
}

fn do_wish(save: &mut SaveData, banner_id: &str, n: u64) {
    if !save.spend(n) {
        screen(
            &format!("{PURPLE}Not enough fates. Top up from the main menu (option 6).{RESET}"),
            &format!("{GREY}( press enter ){RESET}"),
        );
        return;
    }
    let banner = data::banner(banner_id);
    let mut rng = thread_rng();

    // Resolve all pulls up front, then save, then play the show.
    let mut results: Vec<(WishOutcome, bool, u32)> = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let outcome = save.pity.roll(banner, &mut rng);
        let is_new = save.count(outcome.item_id) == 0;
        save.record(outcome.item_id, outcome.rarity as u8, banner_id);
        let count = save.count(outcome.item_id);
        if outcome.rarity == Rarity::Five {
            save.total_5star += 1;
            if outcome.featured {
                save.total_featured_5star += 1;
            }
        }
        results.push((outcome, is_new, count));
    }
    save.save();

    let fast = save.settings.fast;
    let bell = save.settings.bell;
    with_stage(fast, bell, |stage| {
        let best = results.iter().map(|(o, _, _)| o.rarity).max().unwrap_or(Rarity::Three);
        stage.build_up(best);
        for (outcome, is_new, count) in &results {
            let item = data::item(outcome.item_id);
            stage.reveal(item, outcome, *is_new, *count);
        }
        let summary: Vec<(&Item, bool)> = results
            .iter()
            .map(|(o, is_new, _)| (data::item(o.item_id), *is_new))
            .collect();
        stage.summary(&summary);
    });
}

// ---------------------------------------------------------------------------
// Non-wish screens
// ---------------------------------------------------------------------------

fn show_collection(save: &SaveData) {
    // All characters (with owned count), then weapons — rendered as a visual
    // pixel-art gallery in the alternate screen.
    let chars: Vec<(&Item, u32)> = data::ROSTER
        .iter()
        .filter(|i| i.is_character())
        .map(|i| (i, save.count(i.id)))
        .collect();
    let weapons: Vec<(&Item, u32)> = data::ROSTER
        .iter()
        .filter(|i| !i.is_character())
        .map(|i| (i, save.count(i.id)))
        .collect();

    let fast = save.settings.fast;
    let bell = save.settings.bell;
    with_stage(fast, bell, |stage| {
        stage.gallery(&chars, &weapons);
    });
}

fn show_details(save: &SaveData) {
    let p = &save.pity;
    let mut s = format!("{GOLD}{BOLD}── WISH DETAILS ──{RESET}\n\n");
    s += &format!("{BOLD}Character Event Wish{RESET}\n\n");
    s += &format!("{CYAN}5★ character{RESET}   base 0.600%   consolidated ~1.6%\n");
    s += &format!("{GREY}soft pity from pull 74, guaranteed at 90{RESET}\n");
    s += &format!("{PURPLE}4★ item{RESET}       base 5.100%   consolidated ~13%\n");
    s += &format!("{GREY}soft pity from pull 9, guaranteed at 10{RESET}\n\n");
    s += &format!("{BOLD}50/50{RESET}  half of 5★ pulls are the featured character.\n");
    s += &format!("{GREY}Lose it and the next 5★ is guaranteed featured.{RESET}\n");
    s += &format!("{BOLD}Capturing Radiance{RESET}  losing streaks boost your featured\n");
    s += &format!("{GREY}win-rate and cap how many 50/50s you can lose in a row.{RESET}\n\n");
    s += &format!("{BOLD}Your state{RESET}\n");
    s += &format!("5★ pity   {}/90\n", p.pulls_since_5);
    s += &format!("4★ pity   {}/10\n", p.pulls_since_4);
    s += &format!("next 5★   {}\n", if p.guaranteed_5 { "GUARANTEED featured" } else { "50/50" });
    s += &format!("next 4★   {}\n", if p.guaranteed_4 { "GUARANTEED rate-up" } else { "50/50" });
    s += &format!("radiance  {} consecutive losses\n", p.radiance_losses);
    s += &format!("total     {} wishes", p.total_pulls);
    screen(&s, &format!("{GREY}( press enter ){RESET}"));
}

fn show_history(save: &SaveData) {
    let mut s = format!("{GOLD}{BOLD}── RECENT WISHES ──{RESET}\n\n");
    if save.history.is_empty() {
        s += &format!("{GREY}No wishes yet.{RESET}");
    }
    for e in save.history.iter().rev().take(20) {
        let item = data::item(&e.id);
        let (col, stars) = match e.rarity {
            5 => (GOLD, "★★★★★"),
            4 => (PURPLE, "★★★★"),
            _ => (CYAN, "★★★"),
        };
        s += &format!(
            "{GREY}#{:<5}{RESET} {col}{:<6}{RESET} {:<20} {GREY}{}{RESET}\n",
            e.pull, stars, item.name, e.banner
        );
    }
    screen(s.trim_end(), &format!("{GREY}( press enter ){RESET}"));
}

fn settings_menu(save: &mut SaveData) {
    loop {
        let mut body = format!("{GOLD}{BOLD}── SETTINGS ──{RESET}\n\n");
        body += &format!("{BOLD}1{RESET}  Terminal bell on 5★   [{}]\n", onoff(save.settings.bell));
        body += &format!("{BOLD}2{RESET}  Fast mode (short anim) [{}]\n", onoff(save.settings.fast));
        body += &format!("{BOLD}0{RESET}  back");
        match screen(&body, "› ").trim() {
            "1" => save.settings.bell = !save.settings.bell,
            "2" => save.settings.fast = !save.settings.fast,
            "0" | "\u{4}" => {
                save.save();
                return;
            }
            _ => {}
        }
        save.save();
    }
}

fn reset_menu(save: &mut SaveData) {
    let body = format!(
        "{PURPLE}{BOLD}Reset ALL progress — pity, inventory, currency?{RESET}\n\n{GREY}Type RESET to confirm.{RESET}"
    );
    let confirmed = screen(&body, "› ").trim() == "RESET";
    let msg = if confirmed {
        *save = SaveData::default();
        save.save();
        format!("{GREY}The slate is clean.{RESET}")
    } else {
        format!("{GREY}Cancelled.{RESET}")
    };
    screen(&msg, &format!("{GREY}( press enter ){RESET}"));
}

// ---------------------------------------------------------------------------
// Terminal helpers
// ---------------------------------------------------------------------------

fn with_stage<F: FnOnce(&mut Stage)>(fast: bool, bell: bool, f: F) {
    let mut out = stdout();
    terminal::enable_raw_mode().ok();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide).ok();

    let mut stage = Stage::new(stdout(), fast, bell);
    f(&mut stage);

    graphics::clear(&mut out).ok();
    execute!(out, cursor::Show, terminal::LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();
}

/// Visible width of a string, ignoring ANSI escape sequences.
fn viz_len(s: &str) -> usize {
    let mut n = 0usize;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip a CSI sequence up to its final letter.
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

/// Clear the whole window and render `body` centered both ways, with `prompt`
/// centered just below it. Reads and returns a line of input (EOT sentinel on
/// end-of-input). Used for every menu and info screen.
fn screen(body: &str, prompt: &str) -> String {
    let (w, h) = terminal::size().unwrap_or((80, 24));
    let lines: Vec<&str> = body.lines().collect();
    let block = lines.len() as u16 + 2; // body + blank + prompt
    let top = h.saturating_sub(block) / 2;

    let mut buf = String::from("\x1b[2J\x1b[H");
    for _ in 0..top {
        buf.push('\n');
    }
    let pad = |s: &str| " ".repeat((w as usize).saturating_sub(viz_len(s)) / 2);
    for l in &lines {
        if !l.is_empty() {
            buf.push_str(&pad(l));
            buf.push_str(l);
        }
        buf.push_str("\r\n");
    }
    buf.push_str("\r\n");
    buf.push_str(&pad(prompt));
    buf.push_str(prompt);
    print!("{buf}");
    stdout().flush().ok();

    let mut s = String::new();
    match std::io::stdin().read_line(&mut s) {
        Ok(0) => "\u{4}".to_string(),
        _ => s,
    }
}

fn onoff(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

// ---------------------------------------------------------------------------
// Headless odds simulation (for verifying the engine)
// ---------------------------------------------------------------------------

fn simulate(n: u64, banner_id: &str) {
    let banner = data::banner(banner_id);
    let mut pity = PityState::default();
    let mut rng = thread_rng();

    let (mut c5, mut c4, mut c3) = (0u64, 0u64, 0u64);
    let mut featured5 = 0u64;
    let mut lost = 0u64;
    let mut pity_sum = 0u64;
    let mut last5 = 0u64;

    for i in 1..=n {
        let o = pity.roll(banner, &mut rng);
        match o.rarity {
            Rarity::Five => {
                c5 += 1;
                pity_sum += i - last5;
                last5 = i;
                if o.featured {
                    featured5 += 1;
                }
                if o.lost_5050 {
                    lost += 1;
                }
            }
            Rarity::Four => c4 += 1,
            Rarity::Three => c3 += 1,
        }
    }

    let pct = |x: u64| x as f64 / n as f64 * 100.0;
    println!("Simulated {n} wishes on '{}'", banner.name);
    println!("  5★ : {c5:>7}  ({:.3}%)", pct(c5));
    println!("  4★ : {c4:>7}  ({:.3}%)", pct(c4));
    println!("  3★ : {c3:>7}  ({:.3}%)", pct(c3));
    if c5 > 0 {
        println!("  avg pity per 5★     : {:.1}", pity_sum as f64 / c5 as f64);
        println!(
            "  featured 5★         : {featured5} / {c5}  ({:.1}% of 5★)",
            featured5 as f64 / c5 as f64 * 100.0
        );
        println!("  lost 50/50          : {lost}");
    }
    println!(
        "  target: 5★ ~1.6%, 4★ ~12.5%, avg pity ~62, featured ~70% (radiance)"
    );
}

fn print_help() {
    println!("wish — a terminal gacha / wish simulator (Genshin-accurate odds)\n");
    println!("USAGE:");
    println!("  wish                     launch the interactive simulator");
    println!("  wish --simulate N [id]   roll N wishes headless, print odds");
    println!("  wish --help              this message\n");
    println!("Banner ids: snowfall, everblaze, starfall");
    println!("Env: WISH_FORCE_GRAPHICS=1 forces image output on unknown terminals.");
}

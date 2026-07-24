//! A terminal gacha / wish simulator with Genshin-accurate odds, Ghostty
//! inline-image reveals, a pixel-art collection, and turn-based battles.

mod anim;
mod art;
mod battle;
mod data;
mod fx;
mod gacha;
mod graphics;
mod model;
mod save;
mod ui;

use std::io::stdout;

use crossterm::terminal;
use crossterm::{cursor, execute};
use rand::thread_rng;

use anim::Stage;
use gacha::{PityState, WishOutcome};
use model::{Item, Rarity};
use save::SaveData;
use ui::{MenuItem, Ui};

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
            let mut items: Vec<serde_json::Value> = data::ROSTER
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
            for e in battle::ENEMIES {
                items.push(serde_json::json!({
                    "id": format!("enemy_{}", e.id),
                    "name": e.name,
                    "kind": "enemy",
                    "element": e.element.name(),
                    "rarity": if e.boss { 5 } else { 4 },
                    "look": e.look,
                }));
            }
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
        Some("--dump-fx") => {
            let dir = args.get(1).map(|s| s.as_str()).unwrap_or("fx-preview");
            std::fs::create_dir_all(dir).ok();
            let frames = fx::wish_cinematic(Rarity::Five);
            for (i, f) in frames.iter().enumerate() {
                std::fs::write(format!("{dir}/frame_{i:02}.png"), f).ok();
            }
            println!("wrote {} frames to {dir}", frames.len());
            return;
        }
        Some("--battle-sim") => {
            let h1 = args.get(1).map(|s| s.as_str()).unwrap_or("yukihana");
            let h2 = args.get(2).map(|s| s.as_str()).unwrap_or("guren");
            let f1 = args.get(3).map(|s| s.as_str()).unwrap_or("slime");
            let f2 = args.get(4).map(|s| s.as_str()).unwrap_or("goblin");
            battle_sim(&[h1, h2], &[f1, f2]);
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
// Interactive session (persistent full-screen widget UI)
// ---------------------------------------------------------------------------

fn interactive() {
    let mut ui = Ui::new();
    let mut save = SaveData::load();

    loop {
        let p = &save.pity;
        let guar5 = if p.guaranteed_5 { "GUARANTEED" } else { "50/50" };
        let header = vec![
            format!(
                "Intertwined Fates {}     Primogems {}     (≈ {} wishes)",
                save.intertwined_fates, save.primogems, save.available_wishes()
            ),
            format!(
                "5★ pity {}/90     4★ pity {}/10     next 5★ {}",
                p.pulls_since_5, p.pulls_since_4, guar5
            ),
        ];
        let items = [
            MenuItem::new("Wish"),
            MenuItem::new("Collection"),
            MenuItem::new("Battle"),
            MenuItem::new("Wish Details"),
            MenuItem::new("History"),
            MenuItem::new("Settings"),
            MenuItem::new("Top Up (sim)"),
            MenuItem::new("Reset"),
            MenuItem::new("Quit"),
        ];
        match ui.menu("✦   W I S H   S I M U L A T O R   ✦", &header, &items, false) {
            Some(0) => wish_menu(&mut ui, &mut save),
            Some(1) => show_collection(&mut ui, &save),
            Some(2) => battle_menu(&mut ui, &mut save),
            Some(3) => show_details(&mut ui, &save),
            Some(4) => show_history(&mut ui, &save),
            Some(5) => settings_menu(&mut ui, &mut save),
            Some(6) => {
                save.primogems += 16000;
                save.intertwined_fates += 10;
                save.save();
                ui.message(&["+16000 primogems, +10 fates.".to_string()], "Top Up");
            }
            Some(7) => reset_menu(&mut ui, &mut save),
            _ => break,
        }
    }
    ui.close();
}

fn wish_menu(ui: &mut Ui, save: &mut SaveData) {
    loop {
        let items: Vec<MenuItem> = data::BANNERS
            .iter()
            .map(|b| {
                let five = data::item(b.featured_5);
                let el = five.element().map(|e| e.name()).unwrap_or("");
                let f4: Vec<&str> = b.featured_4.iter().map(|id| data::item(id).name).collect();
                MenuItem::new(b.name).detail(vec![
                    format!("★★★★★  {}   {}  ·  {}", five.name, el, five.title),
                    format!("★★★★   {}", f4.join(", ")),
                ])
            })
            .collect();
        let header = vec![format!("≈ {} wishes available", save.available_wishes())];
        match ui.menu("CHOOSE A BANNER", &header, &items, true) {
            Some(i) => count_menu(ui, save, data::BANNERS[i].id),
            None => return,
        }
    }
}

fn count_menu(ui: &mut Ui, save: &mut SaveData, banner_id: &str) {
    let name = data::banner(banner_id).name;
    let items = [
        MenuItem::new("Wish ×1    (1 fate)"),
        MenuItem::new("Wish ×10   (10 fates)"),
    ];
    let header = vec![format!("≈ {} wishes available", save.available_wishes())];
    match ui.menu(name, &header, &items, true) {
        Some(0) => do_wish(ui, save, banner_id, 1),
        Some(1) => do_wish(ui, save, banner_id, 10),
        _ => {}
    }
}

fn do_wish(ui: &mut Ui, save: &mut SaveData, banner_id: &str, n: u64) {
    if !save.spend(n) {
        ui.message(
            &["Not enough fates.".to_string(), "Use Top Up from the main menu.".to_string()],
            "—",
        );
        return;
    }
    let banner = data::banner(banner_id);
    let mut rng = thread_rng();

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

    let (fast, bell) = (save.settings.fast, save.settings.bell);
    run_stage(fast, bell, |stage| {
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

fn show_collection(_ui: &mut Ui, save: &SaveData) {
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
    let (fast, bell) = (save.settings.fast, save.settings.bell);
    run_stage(fast, bell, |stage| {
        stage.gallery(&chars, &weapons);
    });
}

fn show_details(ui: &mut Ui, save: &SaveData) {
    let p = &save.pity;
    let lines = vec![
        "Character Event Wish".to_string(),
        String::new(),
        "5★ character   base 0.600%   consolidated ~1.6%".to_string(),
        "soft pity from pull 74, guaranteed at 90".to_string(),
        "4★ item        base 5.100%   consolidated ~12.5%".to_string(),
        "soft pity from pull 9, guaranteed at 10".to_string(),
        String::new(),
        "50/50 — half of 5★ pulls are the featured character.".to_string(),
        "Capturing Radiance boosts your featured win-rate on losing streaks.".to_string(),
        String::new(),
        format!("5★ pity {}/90     4★ pity {}/10", p.pulls_since_5, p.pulls_since_4),
        format!(
            "next 5★ {}     radiance {} losses     total {} wishes",
            if p.guaranteed_5 { "GUARANTEED" } else { "50/50" },
            p.radiance_losses,
            p.total_pulls
        ),
    ];
    ui.message(&lines, "WISH DETAILS");
}

fn show_history(ui: &mut Ui, save: &SaveData) {
    let mut lines = Vec::new();
    if save.history.is_empty() {
        lines.push("No wishes yet.".to_string());
    }
    for e in save.history.iter().rev().take(16) {
        let item = data::item(&e.id);
        let stars = "★".repeat(e.rarity as usize);
        lines.push(format!("#{:<5} {:<6} {:<20} {}", e.pull, stars, item.name, e.banner));
    }
    ui.message(&lines, "RECENT WISHES");
}

fn settings_menu(ui: &mut Ui, save: &mut SaveData) {
    loop {
        let items = [
            MenuItem::new(format!("Terminal bell on 5★   [{}]", onoff(save.settings.bell))),
            MenuItem::new(format!("Fast mode (short anim) [{}]", onoff(save.settings.fast))),
            MenuItem::new("Back"),
        ];
        match ui.menu("SETTINGS", &[], &items, true) {
            Some(0) => save.settings.bell = !save.settings.bell,
            Some(1) => save.settings.fast = !save.settings.fast,
            _ => {
                save.save();
                return;
            }
        }
        save.save();
    }
}

fn reset_menu(ui: &mut Ui, save: &mut SaveData) {
    let warn = vec![
        "This erases pity, inventory, and currency.".to_string(),
        "There is no undo.".to_string(),
    ];
    if ui.confirm("Reset ALL progress?", &warn) {
        *save = SaveData::default();
        save.save();
        ui.message(&["The slate is clean.".to_string()], "Reset");
    }
}

// ---------------------------------------------------------------------------
// Battle
// ---------------------------------------------------------------------------

fn battle_menu(ui: &mut Ui, save: &mut SaveData) {
    let owned: Vec<&Item> = data::ROSTER
        .iter()
        .filter(|i| i.is_character() && save.count(i.id) > 0)
        .collect();
    if owned.is_empty() {
        ui.message(
            &["You have no characters yet.".to_string(), "Wish on a banner first!".to_string()],
            "Battle",
        );
        return;
    }

    let hero_items: Vec<MenuItem> = owned
        .iter()
        .map(|i| {
            let el = i.element().map(|e| e.name()).unwrap_or("");
            MenuItem::new(format!("{}  {}", i.name, el))
                .detail(vec![format!("{}  ·  {}", "★".repeat(i.rarity.stars()), i.title)])
        })
        .collect();
    let want = owned.len().min(2);
    let hero_pick = ui.multi_select(
        "CHOOSE YOUR TEAM",
        &[format!("Pick {want} fighter(s) for the 2v2")],
        &hero_items,
        want,
    );
    if hero_pick.is_empty() {
        return;
    }
    let heroes: Vec<&Item> = hero_pick.iter().map(|&i| owned[i]).collect();

    let enemy_items: Vec<MenuItem> = battle::ENEMIES
        .iter()
        .map(|e| {
            let label = if e.boss { format!("{}  (BOSS)", e.name) } else { e.name.to_string() };
            MenuItem::new(label).detail(vec![format!(
                "{}  ·  reward {} primogems",
                e.element.name(),
                e.reward
            )])
        })
        .collect();
    let foe_pick = ui.multi_select(
        "CHOOSE 2 OPPONENTS",
        &["Pick 2 enemies to face".to_string()],
        &enemy_items,
        2,
    );
    if foe_pick.is_empty() {
        return;
    }
    let foes: Vec<&battle::Enemy> = foe_pick.iter().map(|&i| &battle::ENEMIES[i]).collect();
    let reward: u64 = foes.iter().map(|e| e.reward).sum();

    let (fast, bell) = (save.settings.fast, save.settings.bell);
    let mut outcome = battle::Outcome::Fled;
    run_stage(fast, bell, |stage| {
        outcome = stage.battle(&heroes, &foes);
    });

    match outcome {
        battle::Outcome::Win => {
            save.primogems += reward;
            save.save();
            ui.message(
                &["Your team is victorious!".to_string(), format!("+{reward} primogems")],
                "VICTORY",
            );
        }
        battle::Outcome::Lose => {
            ui.message(&["Your team was defeated…".to_string()], "DEFEAT");
        }
        battle::Outcome::Fled => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a `Stage`-based full-screen sequence. Assumes the `Ui` session already
/// holds raw mode + the alternate screen; just clears any inline images after.
fn run_stage<F: FnOnce(&mut Stage)>(fast: bool, bell: bool, f: F) {
    let mut out = stdout();
    let mut stage = Stage::new(stdout(), fast, bell);
    f(&mut stage);
    graphics::clear(&mut out).ok();
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
    println!("  target: 5★ ~1.6%, 4★ ~12.5%, avg pity ~62, featured ~70% (radiance)");
}

/// Headless 2v2 auto-battle to verify the team mechanics (both sides AI).
fn battle_sim(hero_ids: &[&str], foe_ids: &[&str]) {
    use battle::Battler;
    let mut heroes: Vec<Battler> = hero_ids.iter().map(|id| battle::player_battler(data::item(id))).collect();
    let mut foes: Vec<Battler> = foe_ids.iter().map(|id| battle::enemy_battler(battle::enemy(id))).collect();
    let mut rng = thread_rng();
    let alive = |t: &[Battler]| (0..t.len()).filter(|&i| t[i].alive()).count();
    let names = |t: &[Battler]| t.iter().map(|b| b.name.clone()).collect::<Vec<_>>().join(" & ");
    println!("{}  vs  {}\n", names(&heroes), names(&foes));

    for round in 1..=60 {
        if alive(&heroes) == 0 || alive(&foes) == 0 {
            break;
        }
        // Gather actions (both sides AI): (is_hero, actor, move, target).
        let mut acts: Vec<(bool, usize, usize, usize)> = Vec::new();
        for i in 0..heroes.len() {
            if heroes[i].alive() {
                let (mi, ti) = battle::ai_action(&heroes[i], &foes, &mut rng);
                acts.push((true, i, mi, ti));
            }
        }
        for i in 0..foes.len() {
            if foes[i].alive() {
                let (mi, ti) = battle::ai_action(&foes[i], &heroes, &mut rng);
                acts.push((false, i, mi, ti));
            }
        }
        acts.sort_by(|a, b| {
            let sa = if a.0 { heroes[a.1].eff_spd() } else { foes[a.1].eff_spd() };
            let sb = if b.0 { heroes[b.1].eff_spd() } else { foes[b.1].eff_spd() };
            sb.partial_cmp(&sa).unwrap()
        });

        for (ah, ai, mi, ti) in acts {
            let actor_alive = if ah { heroes[ai].alive() } else { foes[ai].alive() };
            if !actor_alive {
                continue;
            }
            let mut log = Vec::new();
            let can = {
                let actor = if ah { &mut heroes[ai] } else { &mut foes[ai] };
                battle::can_act(actor, &mut rng, &mut log)
            };
            if can {
                let support = battle::is_support(&(if ah { &heroes[ai] } else { &foes[ai] }).moves[mi]);
                if support {
                    let actor = if ah { &mut heroes[ai] } else { &mut foes[ai] };
                    log.extend(battle::apply_support(actor, mi));
                } else {
                    let tgt_alive = if ah { foes[ti].alive() } else { heroes[ti].alive() };
                    let rti = if tgt_alive {
                        Some(ti)
                    } else if ah {
                        (0..foes.len()).find(|&j| foes[j].alive())
                    } else {
                        (0..heroes.len()).find(|&j| heroes[j].alive())
                    };
                    if let Some(ti) = rti {
                        if ah {
                            log.extend(battle::apply_move(&mut heroes[ai], &mut foes[ti], mi, &mut rng));
                        } else {
                            log.extend(battle::apply_move(&mut foes[ai], &mut heroes[ti], mi, &mut rng));
                        }
                    }
                }
            }
            for l in &log {
                println!("{l}");
            }
            if alive(&heroes) == 0 || alive(&foes) == 0 {
                break;
            }
        }
        let mut log = Vec::new();
        for b in heroes.iter_mut().chain(foes.iter_mut()) {
            battle::end_of_round(b, &mut log);
        }
        for l in &log {
            println!("{l}");
        }
        let hp = |t: &[Battler]| t.iter().map(|b| format!("{} {}/{}", b.name, b.hp, b.max_hp)).collect::<Vec<_>>().join(", ");
        println!("  round {round}:  [{}]  vs  [{}]", hp(&heroes), hp(&foes));
    }
    let winner = if alive(&heroes) > 0 { "Heroes" } else { "Enemies" };
    println!("\nWinner: {winner}");
}

fn print_help() {
    println!("wish — a terminal gacha / wish simulator (Genshin-accurate odds)\n");
    println!("USAGE:");
    println!("  wish                     launch the interactive game");
    println!("  wish --simulate N [id]   roll N wishes headless, print odds");
    println!("  wish --export-prompts    dump art prompts as JSON");
    println!("  wish --help              this message\n");
    println!("Banner ids: snowfall, everblaze, starfall, verdant");
    println!("Env: WISH_FORCE_GRAPHICS=1 forces image output on unknown terminals.");
}

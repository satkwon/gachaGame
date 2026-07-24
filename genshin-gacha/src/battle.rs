//! A compact turn-based battle system, Pokémon-flavored: HP + MP, elemental
//! moves, a type chart, and status effects (poison / burn / freeze / paralyze).
//!
//! Pure logic + data only — the UI lives in `anim.rs`. Characters fight one of
//! the classic-fantasy enemies below; each side acts once per round in speed
//! order.

use rand::Rng;

use crate::model::{Element, Item, Rgb};

// ---------------------------------------------------------------------------
// Moves, effects, status
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Poison,
    Burn,
    Freeze,
    Paralyze,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Poison => "Poisoned",
            Status::Burn => "Burned",
            Status::Freeze => "Frozen",
            Status::Paralyze => "Paralyzed",
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Status::Poison => "PSN",
            Status::Burn => "BRN",
            Status::Freeze => "FRZ",
            Status::Paralyze => "PAR",
        }
    }
    pub fn color(self) -> Rgb {
        match self {
            Status::Poison => Rgb::new(170, 110, 220),
            Status::Burn => Rgb::new(240, 120, 70),
            Status::Freeze => Rgb::new(140, 210, 255),
            Status::Paralyze => Rgb::new(240, 210, 80),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Stat {
    Atk,
    Def,
    Spd,
}

#[derive(Clone, Copy)]
pub enum Effect {
    None,
    Inflict(Status),
    Lower(Stat),
    RaiseSelf(Stat),
    HealSelf(u8), // percent of max HP
}

#[derive(Clone)]
pub struct MoveDef {
    pub name: &'static str,
    pub mp: i32,
    pub power: i32,
    pub element: Element,
    pub effect: Effect,
    pub effect_chance: u8,
}

fn element_names(e: Element) -> (&'static str, &'static str) {
    match e {
        Element::Pyro => ("Ember Burst", "Scorch"),
        Element::Hydro => ("Tide Crash", "Riptide"),
        Element::Cryo => ("Frost Lance", "Rime"),
        Element::Electro => ("Thunderclap", "Static Snare"),
        Element::Anemo => ("Gale Slash", "Cyclone"),
        Element::Dendro => ("Thornvolley", "Blight"),
        Element::Geo => ("Stone Fist", "Bulwark"),
    }
}

fn offensive_effect(e: Element) -> Effect {
    match e {
        Element::Pyro => Effect::Inflict(Status::Burn),
        Element::Cryo => Effect::Inflict(Status::Freeze),
        Element::Electro => Effect::Inflict(Status::Paralyze),
        Element::Dendro => Effect::Inflict(Status::Poison),
        Element::Hydro | Element::Geo => Effect::Lower(Stat::Def),
        Element::Anemo => Effect::Lower(Stat::Spd),
    }
}

fn skill_effect(e: Element) -> (Effect, i32) {
    // (effect, power). Geo braces (0 power, self buff); others hit + status/debuff.
    match e {
        Element::Geo => (Effect::RaiseSelf(Stat::Def), 0),
        _ => (offensive_effect(e), 42),
    }
}

/// The shared four-slot moveset, themed by element.
pub fn moves_for(e: Element) -> Vec<MoveDef> {
    let (special, skill) = element_names(e);
    let (skill_fx, skill_pow) = skill_effect(e);
    vec![
        MoveDef { name: "Strike", mp: 0, power: 35, element: e, effect: Effect::None, effect_chance: 0 },
        MoveDef { name: special, mp: 18, power: 68, element: e, effect: offensive_effect(e), effect_chance: 40 },
        MoveDef { name: skill, mp: 14, power: skill_pow, element: e, effect: skill_fx, effect_chance: 100 },
        MoveDef { name: "Mend", mp: 20, power: 0, element: e, effect: Effect::HealSelf(40), effect_chance: 100 },
    ]
}

// ---------------------------------------------------------------------------
// Type chart
// ---------------------------------------------------------------------------

pub fn type_mult(a: Element, d: Element) -> f32 {
    use Element::*;
    let strong = matches!(
        (a, d),
        (Pyro, Cryo) | (Pyro, Dendro)
            | (Hydro, Pyro) | (Hydro, Geo)
            | (Cryo, Hydro) | (Cryo, Dendro)
            | (Electro, Hydro)
            | (Geo, Electro)
            | (Dendro, Hydro) | (Dendro, Geo)
    );
    let weak = matches!(
        (a, d),
        (Pyro, Hydro)
            | (Hydro, Cryo) | (Hydro, Dendro)
            | (Cryo, Pyro)
            | (Electro, Geo)
            | (Geo, Hydro)
            | (Dendro, Pyro) | (Dendro, Cryo)
    );
    if strong {
        1.5
    } else if weak {
        0.75
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// Battler
// ---------------------------------------------------------------------------

pub struct Battler {
    pub name: String,
    pub sprite: String, // assets/pixel/<sprite>.png
    pub element: Element,
    pub max_hp: i32,
    pub hp: i32,
    pub max_mp: i32,
    pub mp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub atk_stage: i32,
    pub def_stage: i32,
    pub spd_stage: i32,
    pub status: Option<Status>,
    pub status_turns: i32,
    pub moves: Vec<MoveDef>,
    pub is_player: bool,
}

fn stage_mult(stage: i32) -> f32 {
    let s = stage.clamp(-3, 3);
    if s >= 0 {
        (2 + s) as f32 / 2.0
    } else {
        2.0 / (2 - s) as f32
    }
}

impl Battler {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }

    fn eff_atk(&self) -> f32 {
        let burn = if self.status == Some(Status::Burn) { 0.5 } else { 1.0 };
        self.atk as f32 * stage_mult(self.atk_stage) * burn
    }
    fn eff_def(&self) -> f32 {
        (self.def as f32 * stage_mult(self.def_stage)).max(1.0)
    }
    pub fn eff_spd(&self) -> f32 {
        let par = if self.status == Some(Status::Paralyze) { 0.5 } else { 1.0 };
        self.spd as f32 * stage_mult(self.spd_stage) * par
    }

    fn stage_mut(&mut self, stat: Stat) -> &mut i32 {
        match stat {
            Stat::Atk => &mut self.atk_stage,
            Stat::Def => &mut self.def_stage,
            Stat::Spd => &mut self.spd_stage,
        }
    }
}

pub fn player_battler(item: &Item) -> Battler {
    let element = item.element().unwrap_or(Element::Anemo);
    let five = item.rarity.stars() == 5;
    let (hp, mp, atk, def, spd) = if five {
        (220, 60, 44, 27, 35)
    } else {
        (185, 52, 37, 23, 31)
    };
    Battler {
        name: item.name.to_string(),
        sprite: item.id.to_string(),
        element,
        max_hp: hp,
        hp,
        max_mp: mp,
        mp,
        atk,
        def,
        spd,
        atk_stage: 0,
        def_stage: 0,
        spd_stage: 0,
        status: None,
        status_turns: 0,
        moves: moves_for(element),
        is_player: true,
    }
}

// ---------------------------------------------------------------------------
// Enemies
// ---------------------------------------------------------------------------

pub struct Enemy {
    pub id: &'static str,
    pub name: &'static str,
    pub element: Element,
    pub hp: i32,
    pub mp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub boss: bool,
    pub reward: u64, // primogems on victory
    pub look: &'static str,
    /// Landscape prompt for this enemy's battle backdrop.
    pub scene: &'static str,
}

pub const ENEMIES: &[Enemy] = &[
    Enemy {
        id: "slime",
        name: "Verdant Slime",
        element: Element::Dendro,
        hp: 150, mp: 40, atk: 30, def: 18, spd: 20, boss: false, reward: 60,
        look: "a cute round green slime monster, glossy translucent body, big simple eyes, \
               small and squishy",
        scene: "a lush green forest clearing with mossy boulders, ferns and dappled sunlight",
    },
    Enemy {
        id: "goblin",
        name: "Goblin Raider",
        element: Element::Geo,
        hp: 175, mp: 40, atk: 36, def: 22, spd: 28, boss: false, reward: 90,
        look: "a small green-skinned goblin, pointy ears, sharp teeth, tattered leather armor, \
               wielding a crude wooden club, menacing grin",
        scene: "a goblin war camp at dusk, crude wooden palisades, tattered banners and campfires",
    },
    Enemy {
        id: "skeleton",
        name: "Skeleton Knight",
        element: Element::Cryo,
        hp: 200, mp: 45, atk: 38, def: 28, spd: 24, boss: false, reward: 120,
        look: "an undead skeleton warrior, glowing blue eye sockets, rusted iron sword and round \
               shield, tattered cloak",
        scene: "an ancient crumbling crypt, stone sarcophagi, cobwebs and cold blue torchlight",
    },
    Enemy {
        id: "flame_imp",
        name: "Flame Imp",
        element: Element::Pyro,
        hp: 165, mp: 55, atk: 42, def: 20, spd: 36, boss: false, reward: 130,
        look: "a small red fire imp demon, little curved horns, pointed tail, hands wreathed in \
               flame, mischievous grin",
        scene: "a volcanic cavern with glowing lava flows, cracked obsidian rock and floating embers",
    },
    Enemy {
        id: "frost_wraith",
        name: "Frost Wraith",
        element: Element::Cryo,
        hp: 190, mp: 60, atk: 40, def: 22, spd: 30, boss: false, reward: 150,
        look: "a ghostly floating frost wraith, tattered pale-blue robes, hollow glowing eyes, \
               icy skeletal claws, drifting cold mist",
        scene: "a frozen wasteland of jagged ice spires and drifting snow beneath a pale aurora",
    },
    Enemy {
        id: "shadow_dragon",
        name: "Shadow Dragon",
        element: Element::Electro,
        hp: 320, mp: 80, atk: 50, def: 32, spd: 34, boss: true, reward: 400,
        look: "a large black shadow dragon, glowing violet eyes, vast tattered wings, crackling \
               dark-purple energy, fearsome and imposing",
        scene: "a vast dragon's lair, cathedral-sized obsidian cavern with a golden treasure hoard and violet glow",
    },
];

pub fn enemy(id: &str) -> &'static Enemy {
    ENEMIES.iter().find(|e| e.id == id).expect("unknown enemy")
}

pub fn enemy_battler(e: &Enemy) -> Battler {
    Battler {
        name: e.name.to_string(),
        sprite: format!("enemy_{}", e.id),
        element: e.element,
        max_hp: e.hp,
        hp: e.hp,
        max_mp: e.mp,
        mp: e.mp,
        atk: e.atk,
        def: e.def,
        spd: e.spd,
        atk_stage: 0,
        def_stage: 0,
        spd_stage: 0,
        status: None,
        status_turns: 0,
        moves: moves_for(e.element),
        is_player: false,
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Whether the battler can act this turn; pushes a message if it can't.
/// Also handles thawing / waking checks. Returns false if the turn is skipped.
pub fn can_act<R: Rng>(b: &mut Battler, rng: &mut R, log: &mut Vec<String>) -> bool {
    match b.status {
        Some(Status::Freeze) => {
            if rng.gen_bool(0.25) {
                b.status = None;
                log.push(format!("{} thawed out!", b.name));
                true
            } else if rng.gen_bool(0.30) {
                log.push(format!("{} is frozen solid!", b.name));
                false
            } else {
                true
            }
        }
        Some(Status::Paralyze) => {
            if rng.gen_bool(0.25) {
                log.push(format!("{} is paralyzed and can't move!", b.name));
                false
            } else {
                true
            }
        }
        _ => true,
    }
}

/// Apply move `mi` from `user` to `target`. Returns log lines.
pub fn apply_move<R: Rng>(
    user: &mut Battler,
    target: &mut Battler,
    mi: usize,
    rng: &mut R,
) -> Vec<String> {
    let mv = user.moves[mi].clone();
    let mut log = Vec::new();
    user.mp = (user.mp - mv.mp).max(0);
    log.push(format!("{} used {}!", user.name, mv.name));

    // Damage.
    if mv.power > 0 {
        let tmult = type_mult(mv.element, target.element);
        let variance = rng.gen_range(0.85..1.0);
        let raw = mv.power as f32 * (user.eff_atk() / target.eff_def()) * tmult * variance;
        let dmg = (raw.round() as i32).max(1);
        target.hp = (target.hp - dmg).max(0);
        log.push(format!("  {} takes {} damage.", target.name, dmg));
        if tmult > 1.0 {
            log.push("  It's super effective!".to_string());
        } else if tmult < 1.0 {
            log.push("  It's not very effective…".to_string());
        }
    }

    // Secondary effect.
    let roll = rng.gen_range(0..100) < mv.effect_chance;
    if roll {
        match mv.effect {
            Effect::None => {}
            Effect::Inflict(s) => {
                if target.alive() && target.status.is_none() {
                    target.status = Some(s);
                    target.status_turns = 4;
                    log.push(format!("  {} is {}!", target.name, s.name()));
                }
            }
            Effect::Lower(stat) => {
                if target.alive() {
                    let st = target.stage_mut(stat);
                    if *st > -3 {
                        *st -= 1;
                        log.push(format!("  {}'s {} fell!", target.name, stat_name(stat)));
                    }
                }
            }
            Effect::RaiseSelf(stat) => {
                let st = user.stage_mut(stat);
                if *st < 3 {
                    *st += 1;
                    log.push(format!("  {}'s {} rose!", user.name, stat_name(stat)));
                }
            }
            Effect::HealSelf(pct) => {
                let heal = (user.max_hp * pct as i32) / 100;
                let before = user.hp;
                user.hp = (user.hp + heal).min(user.max_hp);
                log.push(format!("  {} recovered {} HP.", user.name, user.hp - before));
            }
        }
    }
    log
}

/// Apply a self-targeting support move (power 0: Mend / self-buff). No target.
pub fn apply_support(user: &mut Battler, mi: usize) -> Vec<String> {
    let mv = user.moves[mi].clone();
    let mut log = Vec::new();
    user.mp = (user.mp - mv.mp).max(0);
    log.push(format!("{} used {}!", user.name, mv.name));
    match mv.effect {
        Effect::RaiseSelf(stat) => {
            let st = user.stage_mut(stat);
            if *st < 3 {
                *st += 1;
                log.push(format!("  {}'s {} rose!", user.name, stat_name(stat)));
            } else {
                log.push("  …but it won't go higher.".to_string());
            }
        }
        Effect::HealSelf(pct) => {
            let heal = (user.max_hp * pct as i32) / 100;
            let before = user.hp;
            user.hp = (user.hp + heal).min(user.max_hp);
            log.push(format!("  {} recovered {} HP.", user.name, user.hp - before));
        }
        _ => {}
    }
    log
}

/// Is this move self-targeting (no opponent needed)?
pub fn is_support(mv: &MoveDef) -> bool {
    mv.power == 0
}

fn stat_name(s: Stat) -> &'static str {
    match s {
        Stat::Atk => "Attack",
        Stat::Def => "Defense",
        Stat::Spd => "Speed",
    }
}

/// End-of-round damage-over-time and status countdown.
pub fn end_of_round(b: &mut Battler, log: &mut Vec<String>) {
    if !b.alive() {
        return;
    }
    match b.status {
        Some(Status::Poison) => {
            let dmg = (b.max_hp / 8).max(1);
            b.hp = (b.hp - dmg).max(0);
            log.push(format!("{} is hurt by poison ({}).", b.name, dmg));
        }
        Some(Status::Burn) => {
            let dmg = (b.max_hp / 16).max(1);
            b.hp = (b.hp - dmg).max(0);
            log.push(format!("{} is hurt by its burn ({}).", b.name, dmg));
        }
        _ => {}
    }
    if b.status.is_some() {
        b.status_turns -= 1;
        if b.status_turns <= 0 {
            if let Some(s) = b.status.take() {
                log.push(format!("{} recovered from being {}.", b.name, s.name()));
            }
        }
    }
}

/// Enemy AI: pick a move index. Heals when low, favors affordable specials.
pub fn ai_choose<R: Rng>(user: &Battler, target: &Battler, rng: &mut R) -> usize {
    // Mend (index 3) when hurt and no cheaper priority.
    if user.hp * 100 / user.max_hp < 35 && user.mp >= user.moves[3].mp && rng.gen_bool(0.6) {
        return 3;
    }
    // Skill/special if target has no status yet and it's affordable.
    let special_ok = user.mp >= user.moves[1].mp;
    let skill_ok = user.mp >= user.moves[2].mp;
    if target.status.is_none() && skill_ok && rng.gen_bool(0.4) {
        return 2;
    }
    if special_ok && rng.gen_bool(0.6) {
        return 1;
    }
    0
}

/// Team-aware AI: pick (move index, target index among `targets`). Targets the
/// lowest-HP living opponent; the move via the usual policy.
pub fn ai_action<R: Rng>(user: &Battler, targets: &[Battler], rng: &mut R) -> (usize, usize) {
    let target = targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.alive())
        .min_by_key(|(_, t)| t.hp)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mi = ai_choose(user, &targets[target], rng);
    (mi, target)
}

pub enum Outcome {
    Win,
    Lose,
    Fled,
}

//! The static game database: characters, weapons, and banners.
//!
//! Three original limited 5-stars headline; a compact standard pool and 4-star
//! pool round out the drop tables so the odds behave exactly like Genshin's
//! character event banner.
//!
//! `look` is the appearance description fed to the local anime model by
//! `--generate-art` (danbooru-style tags work best with Animagine XL).

use crate::model::*;

const fn theme(deep: Rgb, mid: Rgb, accent: Rgb) -> Theme {
    Theme { deep, mid, accent }
}

pub const ROSTER: &[Item] = &[
    // ---- Limited 5-star characters -------------------------------------
    Item {
        id: "yukihana",
        name: "Yukihana",
        title: "Blade of the First Snow",
        rarity: Rarity::Five,
        kind: ItemKind::Character { element: Element::Cryo, weapon: WeaponType::Sword },
        limited: true,
        theme: theme(Rgb::new(14, 28, 54), Rgb::new(96, 176, 232), Rgb::new(214, 244, 255)),
        flavor: "Where her sword falls, winter answers — silent, and without mercy.",
        look: "1girl, solo, very long straight white hair, pale blue eyes, hair ornament, \
               elegant ornate white and ice-blue dress, gold filigree trim, detached sleeves, \
               holding a glowing crystalline ice sword, falling snow, calm serious expression",
    },
    Item {
        id: "guren",
        name: "Guren",
        title: "The Ember Sovereign",
        rarity: Rarity::Five,
        kind: ItemKind::Character { element: Element::Pyro, weapon: WeaponType::Catalyst },
        limited: true,
        theme: theme(Rgb::new(46, 10, 14), Rgb::new(226, 74, 58), Rgb::new(255, 214, 128)),
        flavor: "A single lotus of flame — and the night forgets it was ever cold.",
        look: "1girl, solo, long crimson red hair, black inner hair, amber eyes, \
               flowing red and gold kimono dress, gold hair ornaments, detached sleeves, \
               a floating flaming lotus of magic beside her, fire particles, confident smile",
    },
    Item {
        id: "yozora",
        name: "Yozora",
        title: "The Midnight Star",
        rarity: Rarity::Five,
        kind: ItemKind::Character { element: Element::Electro, weapon: WeaponType::Bow },
        limited: true,
        theme: theme(Rgb::new(22, 14, 46), Rgb::new(150, 108, 240), Rgb::new(224, 196, 255)),
        flavor: "She looses one arrow of starlight, and constellations rearrange to follow.",
        look: "1girl, solo, very long purple hair, twintails, glowing violet eyes, \
               star hair ornament, indigo and starlight themed outfit, constellation patterns, \
               holding a glowing electro energy bow, night sky, electric sparks",
    },
    // ---- Standard-pool 5-star character (the 50/50 loser) --------------
    Item {
        id: "celestine",
        name: "Celestine",
        title: "Tidewalker of the Pale Coast",
        rarity: Rarity::Five,
        kind: ItemKind::Character { element: Element::Hydro, weapon: WeaponType::Sword },
        limited: false,
        theme: theme(Rgb::new(10, 30, 46), Rgb::new(70, 176, 220), Rgb::new(198, 240, 255)),
        flavor: "The tide has a memory, and it remembers everyone who wronged it.",
        look: "1girl, solo, long flowing teal hair, blue eyes, elegant pale blue water-themed \
               dress, pearl accents, holding a shimmering blade of water, water splashes, \
               serene expression",
    },
    // ---- Standard-pool 5-star weapon ----------------------------------
    Item {
        id: "celestial_edge",
        name: "Celestial Edge",
        title: "Sword",
        rarity: Rarity::Five,
        kind: ItemKind::Weapon { weapon: WeaponType::Sword },
        limited: false,
        theme: theme(Rgb::new(30, 24, 46), Rgb::new(180, 170, 240), Rgb::new(255, 246, 210)),
        flavor: "Forged from a fallen star; it still remembers the sky.",
        look: "an ornate celestial longsword, glowing blue-white blade, star and constellation \
               motifs, golden hilt with gemstone, radiant particles",
    },
    // ---- 4-star characters (banner rate-ups) ---------------------------
    Item {
        id: "mizuki",
        name: "Mizuki",
        title: "Rainfall Sentinel",
        rarity: Rarity::Four,
        kind: ItemKind::Character { element: Element::Hydro, weapon: WeaponType::Polearm },
        limited: false,
        theme: theme(Rgb::new(16, 32, 48), Rgb::new(96, 180, 224), Rgb::new(206, 240, 255)),
        flavor: "Steady as a downpour, patient as a flood.",
        look: "1girl, solo, blue hair, high ponytail, blue eyes, blue and white sentinel uniform, \
               holding a water polearm spear, rain, calm expression",
    },
    Item {
        id: "tsubaki",
        name: "Tsubaki",
        title: "Petalfall Duelist",
        rarity: Rarity::Four,
        kind: ItemKind::Character { element: Element::Anemo, weapon: WeaponType::Sword },
        limited: false,
        theme: theme(Rgb::new(16, 34, 30), Rgb::new(108, 206, 176), Rgb::new(214, 255, 242)),
        flavor: "Each strike lands like a falling flower — and cuts twice as deep.",
        look: "1girl, solo, dark green hair, green eyes, elegant green duelist outfit, \
               holding a slender sword, falling flower petals, poised stance",
    },
    Item {
        id: "hotaru",
        name: "Hotaru",
        title: "Firefly of the Storm",
        rarity: Rarity::Four,
        kind: ItemKind::Character { element: Element::Electro, weapon: WeaponType::Catalyst },
        limited: false,
        theme: theme(Rgb::new(22, 16, 44), Rgb::new(160, 116, 238), Rgb::new(226, 202, 255)),
        flavor: "A small light, right before the thunder.",
        look: "1girl, solo, short purple hair, bright purple eyes, small mage outfit with cape, \
               floating glowing firefly lights, electric sparks, energetic expression",
    },
    // ---- 4-star weapons (non-featured 4-star pool) ---------------------
    Item {
        id: "whispering_gale",
        name: "Whispering Gale",
        title: "Catalyst",
        rarity: Rarity::Four,
        kind: ItemKind::Weapon { weapon: WeaponType::Catalyst },
        limited: false,
        theme: theme(Rgb::new(16, 32, 30), Rgb::new(120, 200, 180), Rgb::new(220, 250, 240)),
        flavor: "Hold it to your ear and it tells you which way the wind will turn.",
        look: "an ornate floating wind catalyst orb, teal and white, swirling wind currents, \
               soft glow",
    },
    Item {
        id: "ironwood_longbow",
        name: "Ironwood Longbow",
        title: "Bow",
        rarity: Rarity::Four,
        kind: ItemKind::Weapon { weapon: WeaponType::Bow },
        limited: false,
        theme: theme(Rgb::new(24, 30, 18), Rgb::new(150, 180, 100), Rgb::new(232, 246, 200)),
        flavor: "Cut from a tree older than the nearest kingdom.",
        look: "an ornate wooden longbow, green and gold, carved leaf and vine motifs, \
               soft glow",
    },
    Item {
        id: "boulderbreaker",
        name: "Boulderbreaker",
        title: "Claymore",
        rarity: Rarity::Four,
        kind: ItemKind::Weapon { weapon: WeaponType::Claymore },
        limited: false,
        theme: theme(Rgb::new(36, 28, 14), Rgb::new(200, 150, 74), Rgb::new(250, 224, 160)),
        flavor: "Swing once. The mountain apologizes.",
        look: "an ornate massive stone greatsword claymore, brown and gold, rugged rocky texture, \
               heavy",
    },
    // ---- 3-star weapons -------------------------------------------------
    Item {
        id: "travelers_blade",
        name: "Traveler's Blade",
        title: "Sword",
        rarity: Rarity::Three,
        kind: ItemKind::Weapon { weapon: WeaponType::Sword },
        limited: false,
        theme: theme(Rgb::new(20, 26, 40), Rgb::new(90, 120, 170), Rgb::new(200, 220, 250)),
        flavor: "Dependable. Unremarkable. Yours.",
        look: "a simple plain steel longsword, leather-wrapped hilt, no ornamentation",
    },
    Item {
        id: "hunters_bow",
        name: "Hunter's Bow",
        title: "Bow",
        rarity: Rarity::Three,
        kind: ItemKind::Weapon { weapon: WeaponType::Bow },
        limited: false,
        theme: theme(Rgb::new(20, 26, 40), Rgb::new(90, 120, 170), Rgb::new(200, 220, 250)),
        flavor: "It has fed more campfires than it has won battles.",
        look: "a simple plain wooden hunting bow, worn leather grip",
    },
];

pub fn item(id: &str) -> &'static Item {
    ROSTER
        .iter()
        .find(|i| i.id == id)
        .unwrap_or_else(|| panic!("unknown item id: {id}"))
}

// ----------------------------------------------------------------------------
// Drop pools (ids), matching Genshin's character event banner structure.
// ----------------------------------------------------------------------------

/// Standard-pool 5-stars (what you get when you LOSE the 50/50).
pub const STANDARD_5: &[&str] = &["celestine", "celestial_edge"];

/// The full 4-star pool. The featured trio (below) is the rate-up half; the
/// remainder (the 4-star weapons) is the losing half of the 4-star 50/50.
pub const POOL_4: &[&str] = &[
    "mizuki",
    "tsubaki",
    "hotaru",
    "whispering_gale",
    "ironwood_longbow",
    "boulderbreaker",
];

/// 3-star weapons.
pub const POOL_3: &[&str] = &["travelers_blade", "hunters_bow"];

/// A limited character banner: one featured 5-star + three rate-up 4-stars.
pub struct Banner {
    pub id: &'static str,
    pub name: &'static str,
    pub featured_5: &'static str,
    pub featured_4: [&'static str; 3],
}

pub const BANNERS: &[Banner] = &[
    Banner {
        id: "snowfall",
        name: "Snowfall Elegy",
        featured_5: "yukihana",
        featured_4: ["mizuki", "tsubaki", "hotaru"],
    },
    Banner {
        id: "everblaze",
        name: "Everblaze Reverie",
        featured_5: "guren",
        featured_4: ["mizuki", "tsubaki", "hotaru"],
    },
    Banner {
        id: "starfall",
        name: "Starfall Nocturne",
        featured_5: "yozora",
        featured_4: ["mizuki", "tsubaki", "hotaru"],
    },
];

pub fn banner(id: &str) -> &'static Banner {
    BANNERS.iter().find(|b| b.id == id).expect("unknown banner")
}

//! Core data types shared across the whole simulator.

use serde::{Deserialize, Serialize};

/// A plain 24-bit color. Used both for terminal styling (crossterm) and for
/// generating procedural portrait art (the `image` crate).
#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Linear blend between two colors. `t` in [0.0, 1.0].
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb::new(l(self.r, other.r), l(self.g, other.g), l(self.b, other.b))
    }

    /// Scale brightness by `f` (clamped).
    pub fn scale(self, f: f32) -> Rgb {
        let s = |c: u8| ((c as f32) * f).clamp(0.0, 255.0) as u8;
        Rgb::new(s(self.r), s(self.g), s(self.b))
    }

    pub fn crossterm(self) -> crossterm::style::Color {
        crossterm::style::Color::Rgb {
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }
}

/// The seven Genshin elements (visions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Element {
    Pyro,
    Hydro,
    Anemo,
    Electro,
    Dendro,
    Cryo,
    Geo,
}

impl Element {
    pub fn name(self) -> &'static str {
        match self {
            Element::Pyro => "Pyro",
            Element::Hydro => "Hydro",
            Element::Anemo => "Anemo",
            Element::Electro => "Electro",
            Element::Dendro => "Dendro",
            Element::Cryo => "Cryo",
            Element::Geo => "Geo",
        }
    }

    /// A single-glyph emblem used in terminal text.
    pub fn glyph(self) -> &'static str {
        match self {
            Element::Pyro => "✦",
            Element::Hydro => "❋",
            Element::Anemo => "❀",
            Element::Electro => "✧",
            Element::Dendro => "❧",
            Element::Cryo => "❈",
            Element::Geo => "◆",
        }
    }

    #[allow(dead_code)] // handy for future element-tinted UI
    pub fn color(self) -> Rgb {
        match self {
            Element::Pyro => Rgb::new(255, 106, 74),
            Element::Hydro => Rgb::new(74, 190, 255),
            Element::Anemo => Rgb::new(116, 224, 187),
            Element::Electro => Rgb::new(198, 130, 255),
            Element::Dendro => Rgb::new(160, 210, 80),
            Element::Cryo => Rgb::new(160, 230, 255),
            Element::Geo => Rgb::new(255, 190, 90),
        }
    }
}

/// Genshin's five weapon types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponType {
    Sword,
    Claymore,
    Polearm,
    Bow,
    Catalyst,
}

impl WeaponType {
    pub fn name(self) -> &'static str {
        match self {
            WeaponType::Sword => "Sword",
            WeaponType::Claymore => "Claymore",
            WeaponType::Polearm => "Polearm",
            WeaponType::Bow => "Bow",
            WeaponType::Catalyst => "Catalyst",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Three = 3,
    Four = 4,
    Five = 5,
}

impl Rarity {
    pub fn stars(self) -> usize {
        self as usize
    }

    /// The signature rarity color revealed during a wish (blue / purple / gold).
    pub fn reveal_color(self) -> Rgb {
        match self {
            Rarity::Three => Rgb::new(90, 150, 255),  // blue
            Rarity::Four => Rgb::new(196, 120, 255),  // purple
            Rarity::Five => Rgb::new(255, 208, 92),   // gold
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Character { element: Element, weapon: WeaponType },
    Weapon { weapon: WeaponType },
}

/// A palette used to theme both the terminal presentation and the procedural
/// splash card for a given item.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub deep: Rgb,    // background low
    pub mid: Rgb,     // background high / glow
    pub accent: Rgb,  // rim light / particles
}

/// A single obtainable character or weapon.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub rarity: Rarity,
    pub kind: ItemKind,
    #[allow(dead_code)] // set per-item; reserved for banner-history labelling
    pub limited: bool,
    pub theme: Theme,
    /// One line of lore, shown during the 5-star cutscene.
    pub flavor: &'static str,
    /// Visual description that drives AI art generation (`--generate-art`).
    pub look: &'static str,
}

impl Item {
    pub fn is_character(&self) -> bool {
        matches!(self.kind, ItemKind::Character { .. })
    }

    pub fn element(&self) -> Option<Element> {
        match self.kind {
            ItemKind::Character { element, .. } => Some(element),
            _ => None,
        }
    }

    pub fn weapon_type(&self) -> WeaponType {
        match self.kind {
            ItemKind::Character { weapon, .. } => weapon,
            ItemKind::Weapon { weapon } => weapon,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        if self.is_character() {
            "Character"
        } else {
            "Weapon"
        }
    }
}

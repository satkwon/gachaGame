//! Persistent player state: pity, inventory, currency, settings and a capped
//! wish history. Stored as JSON under the user's home directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::gacha::PityState;

pub const FATE_COST_PRIMOGEMS: u64 = 160;
const HISTORY_CAP: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// Ring the terminal bell on a 5-star.
    pub bell: bool,
    /// Skip the slow build-up animation.
    pub fast: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { bell: true, fast: false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub rarity: u8,
    pub banner: String,
    pub pull: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveData {
    pub pity: PityState,
    /// id -> number obtained (constellations / refinements).
    pub inventory: BTreeMap<String, u32>,
    pub intertwined_fates: u64,
    pub primogems: u64,
    pub total_5star: u64,
    pub total_featured_5star: u64,
    pub history: Vec<HistoryEntry>,
    pub settings: Settings,
}

impl Default for SaveData {
    fn default() -> Self {
        SaveData {
            pity: PityState::default(),
            inventory: BTreeMap::new(),
            intertwined_fates: 20,
            primogems: 1600,
            total_5star: 0,
            total_featured_5star: 0,
            history: Vec::new(),
            settings: Settings::default(),
        }
    }
}

impl SaveData {
    pub fn path() -> PathBuf {
        let mut dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push(".genshin-gacha-sim");
        std::fs::create_dir_all(&dir).ok();
        dir.push("save.json");
        dir
    }

    pub fn load() -> SaveData {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => SaveData::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Ok(text) = serde_json::to_string_pretty(self) {
            std::fs::write(path, text).ok();
        }
    }

    /// Total wishes available given fates + convertible primogems.
    pub fn available_wishes(&self) -> u64 {
        self.intertwined_fates + self.primogems / FATE_COST_PRIMOGEMS
    }

    /// Spend `n` wishes, drawing from fates first then converting primogems.
    /// Returns false if there aren't enough.
    pub fn spend(&mut self, n: u64) -> bool {
        if self.available_wishes() < n {
            return false;
        }
        if self.intertwined_fates >= n {
            self.intertwined_fates -= n;
        } else {
            let short = n - self.intertwined_fates;
            self.intertwined_fates = 0;
            self.primogems -= short * FATE_COST_PRIMOGEMS;
        }
        true
    }

    pub fn record(&mut self, id: &str, rarity: u8, banner: &str) {
        *self.inventory.entry(id.to_string()).or_insert(0) += 1;
        self.history.push(HistoryEntry {
            id: id.to_string(),
            rarity,
            banner: banner.to_string(),
            pull: self.pity.total_pulls,
        });
        if self.history.len() > HISTORY_CAP {
            let excess = self.history.len() - HISTORY_CAP;
            self.history.drain(0..excess);
        }
    }

    pub fn count(&self, id: &str) -> u32 {
        self.inventory.get(id).copied().unwrap_or(0)
    }
}

//! The wish engine. Probabilities and pity mirror Genshin Impact's Character
//! Event Wish banner.
//!
//! Reference model (community-verified consolidated rates):
//!   * 5-star base rate 0.6%. Soft pity begins at pull 74, adding +6% per pull,
//!     reaching a guaranteed 5-star at pull 90.
//!   * 4-star base rate 5.1%. Soft pity at pull 9, guaranteed 4-star at pull 10.
//!   * 5-star 50/50: half the time the 5-star is the featured character. Losing
//!     it guarantees the next 5-star is featured.
//!   * 4-star 50/50: same idea for the three rate-up 4-stars.
//!   * Capturing Radiance: after losing the 50/50, the featured-win chance is
//!     boosted on subsequent 5-stars and is capped so you can't keep losing.
//!     (HoYoverse's exact table is hidden; the escalation below approximates it.)

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::data::{self, Banner};
use crate::model::Rarity;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PityState {
    pub pulls_since_5: u32,
    pub pulls_since_4: u32,
    pub guaranteed_5: bool,
    pub guaranteed_4: bool,
    /// Consecutive lost 50/50s, drives Capturing Radiance.
    pub radiance_losses: u32,
    pub total_pulls: u64,
}

impl Default for PityState {
    fn default() -> Self {
        PityState {
            pulls_since_5: 0,
            pulls_since_4: 0,
            guaranteed_5: false,
            guaranteed_4: false,
            radiance_losses: 0,
            total_pulls: 0,
        }
    }
}

/// Escalating featured-win probability for Capturing Radiance, indexed by the
/// number of 50/50s lost in a row. Index 3+ is a guaranteed win.
const RADIANCE_WIN_CHANCE: &[f64] = &[0.50, 0.60, 0.80, 1.00];

fn probability_5(pity: u32) -> f64 {
    // `pity` is the 1-based pull index within the current 5-star streak.
    if pity < 74 {
        0.006
    } else if pity < 90 {
        0.006 + 0.06 * (pity - 73) as f64
    } else {
        1.0
    }
}

fn probability_4(pity: u32) -> f64 {
    if pity < 9 {
        0.051
    } else if pity < 10 {
        0.051 + 0.51 * (pity - 8) as f64
    } else {
        1.0
    }
}

#[derive(Clone, Debug)]
pub struct WishOutcome {
    pub item_id: &'static str,
    pub rarity: Rarity,
    /// True when a rate-up (featured) item was obtained.
    pub featured: bool,
    /// True when this 5-star was a lost 50/50.
    pub lost_5050: bool,
    /// True when Capturing Radiance rescued this featured 5-star.
    pub radiance: bool,
}

impl PityState {
    /// Resolve a single wish on `banner`, advancing pity state in place.
    pub fn roll<R: Rng>(&mut self, banner: &Banner, rng: &mut R) -> WishOutcome {
        self.pulls_since_5 += 1;
        self.pulls_since_4 += 1;
        self.total_pulls += 1;

        if rng.gen::<f64>() < probability_5(self.pulls_since_5) {
            let out = self.decide_five_star(banner, rng);
            self.pulls_since_5 = 0;
            self.pulls_since_4 = 0; // a 5-star also resets 4-star pity
            return out;
        }

        if rng.gen::<f64>() < probability_4(self.pulls_since_4) || self.pulls_since_4 >= 10 {
            let out = self.decide_four_star(banner, rng);
            self.pulls_since_4 = 0;
            return out;
        }

        // 3-star weapon.
        let id = *pick(data::POOL_3, rng);
        WishOutcome {
            item_id: id,
            rarity: Rarity::Three,
            featured: false,
            lost_5050: false,
            radiance: false,
        }
    }

    fn decide_five_star<R: Rng>(&mut self, banner: &Banner, rng: &mut R) -> WishOutcome {
        // A guaranteed pull (owed from a previously lost 50/50) is not itself a
        // 50/50, so it doesn't touch the Capturing Radiance streak counter.
        if self.guaranteed_5 {
            self.guaranteed_5 = false;
            return WishOutcome {
                item_id: banner.featured_5,
                rarity: Rarity::Five,
                featured: true,
                lost_5050: false,
                radiance: false,
            };
        }

        // A fresh 50/50, its win chance escalated by consecutive prior losses.
        let idx = (self.radiance_losses as usize).min(RADIANCE_WIN_CHANCE.len() - 1);
        let win_chance = RADIANCE_WIN_CHANCE[idx];
        let roll = rng.gen::<f64>();

        if roll < win_chance {
            // Won. If the boost (not the base 50%) is what carried it, that's a
            // Capturing Radiance moment.
            let by_radiance = win_chance > 0.5 && roll >= 0.5;
            self.radiance_losses = 0;
            WishOutcome {
                item_id: banner.featured_5,
                rarity: Rarity::Five,
                featured: true,
                lost_5050: false,
                radiance: by_radiance,
            }
        } else {
            // Lost the 50/50: standard-pool 5-star; the streak persists so the
            // next *fresh* 50/50 is more likely to be rescued by radiance.
            self.guaranteed_5 = true;
            self.radiance_losses += 1;
            let id = *pick(data::STANDARD_5, rng);
            WishOutcome {
                item_id: id,
                rarity: Rarity::Five,
                featured: false,
                lost_5050: true,
                radiance: false,
            }
        }
    }

    fn decide_four_star<R: Rng>(&mut self, banner: &Banner, rng: &mut R) -> WishOutcome {
        if self.guaranteed_4 {
            self.guaranteed_4 = false;
            let id = *pick(&banner.featured_4, rng);
            return WishOutcome {
                item_id: id,
                rarity: Rarity::Four,
                featured: true,
                lost_5050: false,
                radiance: false,
            };
        }

        if rng.gen::<bool>() {
            let id = *pick(&banner.featured_4, rng);
            WishOutcome {
                item_id: id,
                rarity: Rarity::Four,
                featured: true,
                lost_5050: false,
                radiance: false,
            }
        } else {
            self.guaranteed_4 = true;
            // Standard half: full 4-star pool minus the currently featured trio.
            let others: Vec<&&str> = data::POOL_4
                .iter()
                .filter(|id| !banner.featured_4.contains(id))
                .collect();
            let id = **pick(&others, rng);
            WishOutcome {
                item_id: id,
                rarity: Rarity::Four,
                featured: false,
                lost_5050: false,
                radiance: false,
            }
        }
    }
}

fn pick<'a, T, R: Rng>(slice: &'a [T], rng: &mut R) -> &'a T {
    &slice[rng.gen_range(0..slice.len())]
}

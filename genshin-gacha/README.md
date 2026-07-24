# Wish Simulator ✦

A terminal gacha / wish simulator modeled on *Genshin Impact*'s Character Event
Wish — the same odds, the same pity, the same 50/50 heartbreak, the streaking
star, the gold flash, and a dedicated cutscene for every 5-star. Built in Rust,
with inline character art rendered through **Ghostty**'s image support.

## Quick start (macOS)

You only need three things to play — the character art is already bundled in
`assets/`, so **no Python or AI models are required**. Just share the whole
`genshin-gacha` folder.

```sh
# 1. Ghostty terminal — needed for the inline character images
#    (needs Homebrew; if you don't have it, see https://brew.sh)
brew install --cask ghostty

# 2. Rust (the toolchain that builds the game)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"          # or just open a new terminal

# 3. Open Ghostty, cd into the folder, and run (first build takes a few minutes)
cd path/to/genshin-gacha
cargo run --release
```

Everything is driven by **buttons**: use **↑ ↓** (or j/k) to move the highlight,
**enter** to select, **esc** to go back, or press an option's **number**. During
a wish, **space** advances and **esc** skips to the results. Any terminal works,
but only Ghostty (or Kitty / WezTerm) shows the inline art.

## The banners

Five limited banners, each headlining an original 5-star:

| Banner | 5★ (limited) | Element · Weapon | Rate-up 4★ |
|---|---|---|---|
| **Snowfall Elegy** | **Yukihana** — *Blade of the First Snow* | Cryo · Sword | Mizuki, Tsubaki, Hotaru |
| **Everblaze Reverie** | **Guren** — *The Ember Sovereign* | Pyro · Catalyst | Mizuki, Tsubaki, Hotaru |
| **Starfall Nocturne** | **Yozora** — *The Midnight Star* | Electro · Bow | Mizuki, Tsubaki, Hotaru |
| **Verdant Reverie** | **Reika** — *The Verdant Oracle* | Dendro · Catalyst | Kogane, Shizuku, Mizuki |
| **Nocturne Requiem** | **Kuroha** — *Nocturne of Black Feathers* | Anemo · Catalyst | Kagerou, Seren, Tsubaki |

The standard pool — 5★ character **Celestine**, 5★ weapon **Celestial Edge**,
three 4★ weapons and two 3★ weapons — fills the losing half of the 50/50.
Thirteen characters and six weapons in all.

## Battle

The **Battle** menu runs a **2v2 (VGC-style)** turn-based fight: pick two of your
characters, pick two classic-fantasy foes (slime, goblin, skeleton, flame imp,
frost wraith, and a shadow-dragon boss), all shown with pixel sprites that
**bounce when they attack**.

- **Team select** — space to toggle each pick, enter to confirm.
- **Per-turn** — choose a move for each living hero, then a target; all four
  combatants act in speed order.
- **HP + MP** — special moves cost MP; every character has a Strike, an elemental
  special, an elemental skill, and a self-heal (Mend).
- **Type chart** — elements are strong/weak against each other (e.g. Pyro melts
  Cryo). "Super effective" hits ×1.5.
- **Status effects** — Burn (damage + weakened attack), Poison (damage each
  turn), Freeze (may skip a turn), Paralyze (slowed, may skip).
- Wipe the enemy team to earn primogems (the sum of both foes' bounties).

## The mechanics (identical to Genshin)

- **5★** base rate 0.6%; soft pity from pull 74 (+6%/pull); guaranteed at 90.
  Consolidated ≈ **1.6%**, average one every **~62** wishes.
- **4★** base rate 5.1%; soft pity at pull 9; guaranteed at pull 10.
- **50/50** — half of 5★ pulls are the featured character; lose it and your
  next 5★ is guaranteed featured. Same 50/50 + guarantee for rate-up 4★.
- **Capturing Radiance** — consecutive lost 50/50s escalate your featured-win
  chance and cap the losing streak (HoYo's exact table is hidden; this
  approximates it, landing featured wins at ~70% of 5★ overall).

Verify the odds yourself, headlessly:

```sh
cargo run --release -- --simulate 1000000 snowfall
```

## The show

Every wish plays out: a hush over a star field → a comet streaks in → the burst
flashes the **rarity colour** (blue / purple / **gold**) before you see what you
got → the portrait appears → stars ignite one by one. A 10-pull reveals each
item in turn.

Every **5-star** triggers a grand cutscene — gold dust, radiant rings, the
portrait rising in, five stars igniting with a bell each, and the character's
title, lore, and 50/50 status ("rate-up WON", "lost the 50/50", or "✦ CAPTURING
RADIANCE ✦").

During any wish, **space** advances to the next item and **esc** skips straight
to the results summary.

## Collection

The **Collection** page (menu option 2) is a select screen: a grid of
**pixel-art character sprites**. Press a **number** to view that character large,
**space** to go back. Unowned characters show as locked, and owned weapons are
listed below.

The sprites are original **Pokémon-style** pixel art — cute full-body,
front-view chibi, standardized framing — generated locally with the
**pixel-art-xl** LoRA over Animagine, then cleaned to a crisp limited palette
(`scripts/generate_pixel.py`). Each is prompted from the character's `look`
(the same description that drives its splash), so it stays inspired by the
splash art rather than downscaled from it. If a sprite is missing the game falls
back to a hand-drawn parametric one.

## The art (local anime model)

Portraits are generated locally with **Animagine XL 4.0** (an open-weight anime
SDXL checkpoint) via Diffusers on Apple Silicon / MPS — no API, no key, and the
finished PNGs live in `assets/portraits/` so the game runs fully offline. If a
portrait is missing the game falls back to a lightweight themed card.

Regenerate (or generate after cloning):

```sh
# one-time setup
conda create -y -n artgen python=3.11
conda run -n artgen pip install -r scripts/requirements.txt

# render every portrait from the game's own prompt data
cargo run --release -- --export-prompts > prompts.json
conda run -n artgen python scripts/generate_art.py            # splash portraits
conda run -n artgen python scripts/generate_art.py yukihana   # just one

# detailed pixel-art collection sprites (pixel-art-xl LoRA; needs peft)
conda run -n artgen pip install peft
conda run -n artgen python scripts/generate_pixel.py          # all characters
```

Each 832×1216 image takes ~1.5–4 min on an M-series Mac. Env knobs:
`GEN_SLICE=0` disables attention slicing (faster, more memory), `GEN_STEPS=N`
sets sampler steps. Prompts come from the `look` field of each entry in
`src/data.rs`, so edit a character there and re-run to reroll its art.

Prefer to hand-place art? Any PNG at `assets/portraits/<id>.png` is used
verbatim — ids are the first field of each `src/data.rs` entry.

## Other commands

```
cargo run --release                 launch the game
cargo run --release -- --simulate N [banner-id]   headless odds check
cargo run --release -- --battle-sim [hero] [foe]  headless battle test
cargo run --release -- --export-prompts           dump art prompts as JSON
cargo run --release -- --dump-pixels [dir]        write pixel-art sprites to PNG
cargo run --release -- --help
```

Save data (pity, inventory, currency) lives in `~/.genshin-gacha-sim/save.json`.
`WISH_FORCE_GRAPHICS=1` forces image output on terminals that aren't detected.

## Project layout

```
src/model.rs     core types (elements, rarities, items, colours)
src/data.rs      the roster + banner definitions
src/gacha.rs     the wish engine (odds, pity, 50/50, radiance)
src/save.rs      persistence, currency
src/graphics.rs  Kitty/Ghostty image protocol
src/art.rs       asset loading, pixel-art sprites, fallback card
src/anim.rs      animations, reveals, cutscene, gallery, battle UI
src/battle.rs    turn-based battle logic (moves, status, type chart, enemies)
src/ui.rs        widget toolkit — button menus, message boxes
src/main.rs      the app: menus, wish flow, battle flow
scripts/         local anime-model art generation (Python/Diffusers)
```

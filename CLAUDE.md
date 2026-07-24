# gachaGame — project guide

A terminal gacha ("wish") simulator with Genshin-accurate odds, AI-generated
anime art rendered inline via Ghostty, a pixel-art collection, and a 2v2
Pokémon-style battle system. Written in Rust.

**The crate lives in `genshin-gacha/`** (this repo root is one level above it).
Run all `cargo` commands from `genshin-gacha/`.

## Build & run

```bash
. "$HOME/.cargo/env"          # cargo is not on PATH by default
cd genshin-gacha
cargo build --release
cargo run --release           # must be run inside Ghostty to see images
```

The binary is `wish`. Save data lives in `~/.genshin-gacha-sim/save.json`.

## Headless commands (how to verify without a TTY)

The interactive UI is raw-mode + alternate-screen, so it **cannot be driven
headlessly** — `event::read()` needs a real terminal. Never try to smoke-test the
TUI by piping stdin; verify with these instead, and inspect rendered PNGs with
the Read tool:

```bash
./target/release/wish --simulate 1000000 snowfall   # gacha odds
./target/release/wish --battle-sim h1 h2 f1 f2      # headless 2v2 battle log
./target/release/wish --export-prompts > prompts.json
./target/release/wish --dump-fx <dir>               # wish cinematic frames
./target/release/wish --dump-fade <id> <dir>        # splash fade-in ladder
./target/release/wish --dump-reveal <dir>           # matted reveal splashes (transparent bg)
./target/release/wish --dump-menu <file.png>        # composited menu backdrop
./target/release/wish --dump-pixels <dir>           # character pixel sprites
```

## Module map (`genshin-gacha/src/`)

| File | Role |
|---|---|
| `model.rs` | Core types: `Element`, `WeaponType`, `Rarity`, `Item`, `Rgb` |
| `data.rs` | The roster (13 characters, 6 weapons) + 5 banners + drop pools |
| `gacha.rs` | Wish engine: rates, pity, 50/50, Capturing Radiance |
| `save.rs` | Persistence, currency, history |
| `battle.rs` | Battle logic + 6 enemies. Pure data/logic, **no IO** |
| `graphics.rs` | Kitty graphics protocol (transmit, frames, delete) |
| `art.rs` | Asset loading, procedural fallback card, pixel sprites, fades, menu backdrop |
| `fx.rs` | Software-rendered wish cinematic (3D warp + rarity burst) |
| `anim.rs` | `Stage`: wish reveals, 5★ cutscene, collection gallery, battle UI |
| `ui.rs` | `Ui`: widget toolkit — button menus, multi-select, message boxes |
| `main.rs` | CLI dispatch + the interactive app flow |

`battle.rs` holds logic only; its UI lives in `anim.rs::battle()`.

## Art pipeline

Art is generated **locally** with Stable Diffusion on Apple Silicon (MPS), then
committed as PNGs so the game runs offline.

```bash
conda run -n artgen python scripts/generate_art.py   <ids…>   # splashes + scenes
conda run -n artgen python scripts/generate_pixel.py <ids…>   # pixel sprites
```

- Env: conda env **`artgen`** (python 3.11, torch + diffusers + peft), already set up.
- Models: `cagliostrolab/animagine-xl-4.0`; pixel sprites add the
  `nerijs/pixel-art-xl` LoRA.
- Prompts come from `prompts.json`, exported from the Rust data
  (`--export-prompts`). Edit the `look` / `scene` fields in `data.rs` /
  `battle.rs`, re-export, then regenerate.

Asset layout (ids match `data.rs` / `battle.rs`):

```
assets/portraits/<id>.png        splash art (characters + weapons)
assets/portraits/menu_*.png      menu key art (Guren / Yukihana action poses)
assets/pixel/<id>.png            character pixel sprites
assets/pixel/enemy_<id>.png      enemy pixel sprites
assets/scenes/scene_<enemy>.png  battle backdrops
```

Missing assets degrade gracefully (procedural fallback card / parametric sprite).

## Hard-won gotchas — read before touching graphics or art

1. **A screen clear drops inline images in Ghostty.** Never `Clear(All)` after
   placing an image you want to keep. To update text over an image, repaint just
   that region with a **solid panel** (`fill` + `btext` in `anim.rs`, `panel` +
   `at_bg` in `ui.rs`). Writing plain spaces punches holes in the image.
2. **Flicker-free animation = double buffering.** Draw the next frame with an
   *alternating image id* at a higher `z`, **then** delete the previous id
   (`draw_png_frame_z` + `delete_image`). Replacing in place with the same id
   flickers; giving every frame a unique id leaks memory and lags.
3. **Never `fuse_lora()` in fp16** — it corrupts colour (everything turns green).
   Apply the LoRA at inference via `cross_attention_kwargs={"scale": …}`.
4. **SDXL loves sheets.** Weapons come out as design sheets and monsters as
   sprite sheets unless you lead with hard singular tokens (`solo`,
   `a single X`, `only one X`) and heavily negate plurals (`sprite sheet,
   reference sheet, multiple, grid, row, lineup, variations`). Characters escape
   this only because `1girl, solo` is a strong danbooru tag.
5. **CLIP truncates at 77 tokens.** Put framing- and identity-critical tokens
   **first**; the tail is silently dropped (a warning appears in the log). This
   is why prompts lead with `full body, front view, centered, …`.
6. **Python `hash()` is per-process randomized**, so `seed_for()` is *not*
   reproducible — re-running produces a different image. Confirm a regeneration
   actually happened by checking **file mtime**, not by eyeballing the image.
7. **`generate_art.py` iterates `prompts.json` order, not argv order.** Once a
   requested weapon silently wasn't rewritten. Always verify mtimes afterwards.
8. **One SDXL job at a time.** 16 GB M3 — two concurrent pipelines OOM.
   Serialize GPU work and do Rust/CPU work in parallel while it runs.
   `GEN_SLICE=0` disables attention slicing (~40% faster, still fits via MPS
   spill). Budget ~3–6 min per image.
9. **Reveal splashes are keyed, not shown raw.** `art::reveal_card_png` runs
   `matte_backdrop` (in `art.rs`): a border flood-fill removes the connected
   background so the card blends into the starfield instead of sitting in a
   rectangle, then a soft themed backlight (`GLOW_*`, the item's `theme.mid`) is
   composited behind the subject (the fade-in ladder and final card share this
   matte). The flood only crosses **bright** pixels (`KEY_LUMA_MIN`) — this is
   load-bearing: without it the colour flood wanders from a dark background into
   a dark outfit and erases it. So only bright backdrops (sky, pale gradients)
   are keyed; a dark backdrop keys too little to pass `KEY_MIN_BG` and falls
   through to a radial vignette, which is fine since dark already blends into the
   dark starfield. It assumes Ghostty composites the transparent PNG over the
   text drawn beneath it (positive z). If a new splash keys badly, tune
   `KEY_TOL` / `KEY_LUMA_MIN` / `KEY_MIN_BG` and preview with `--dump-reveal`.

## Conventions

- Match surrounding style; comments explain *why*, not *what*.
- Keep `battle.rs` IO-free so it stays testable via `--battle-sim`.
- New characters: add to `data.rs` (with `look`), add to a pool/banner,
  re-export prompts, generate a splash **and** a pixel sprite.
- Timing knobs: wish cinematic pacing in `anim.rs::build_up_cinematic`, battle
  message pacing via `beat(…)`, fade speed in `fade_in_portrait`.

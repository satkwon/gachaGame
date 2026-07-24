# Work plan / status

Snapshot for picking the project up in a new session. See `CLAUDE.md` for
architecture, commands, and the graphics/art gotchas.

## Current state

Everything below is **implemented, building clean, and verified** as far as it
can be without a live Ghostty terminal.

**Gacha** — Genshin-accurate Character Event Wish. Verified over 1M pulls:
5★ 1.61% consolidated, average pity 62, 4★ ~12.3%, featured ~70%.
Soft pity 74 / hard 90, 4★ soft 9 / guaranteed 10, 50/50 + guarantee, and
Capturing Radiance (escalating win-rate; the counter persists across guarantee
cycles — an early bug was it being reset, making it dead code).

**Roster** — 13 characters, 6 weapons, 5 banners, 6 enemies.
Limited 5★: Yukihana (Cryo), Guren (Pyro), Yozora (Electro), Reika (Dendro),
Kuroha (Anemo). Standard 5★: Celestine. 4★: Mizuki, Tsubaki, Hotaru, Kogane,
Shizuku, Kagerou, Seren.

**Battle** — 2v2 VGC-style. HP/MP, 7-element type chart, four status effects
(Burn/Poison/Freeze/Paralyze), per-hero move + target selection, speed-ordered
turns, retargeting when a foe faints, team-wipe win/lose, enemy AI that heals
when low. Sprites bounce when attacking. Per-enemy backdrops. Verified with
`--battle-sim`.

**Presentation** — widget UI (arrow/enter/esc button menus, multi-select),
3D warp + rarity-burst wish cinematic (cached per rarity, double-buffered),
splash fade-in dissolve, 5★ cutscene, pixel-art collection with an arcade-style
select screen, and a menu backdrop of Guren vs Yukihana in splash art.

**Assets** — 21 splashes, 19 pixel sprites, 6 battle scenes, all generated
locally and committed.

## Not yet verified (needs a real Ghostty run)

None of this can be checked headlessly — worth a pass on a real terminal:

- Wish cinematic: flicker (double-buffer fix), pacing, the fade-in dissolve.
- Reveal splashes are now matted so they blend into the starfield (like the menu
  art) instead of showing as a solid rectangle: characters fade at the outskirts
  (interior kept intact) and weapons are keyed free to float. Confirm Ghostty
  composites the transparent PNG over the ASCII stars / gold dust as expected,
  and that it reads cleanly at cell size. Preview headlessly with `--dump-reveal`.
- Sprite attack bounce — relies on re-placing an image at a new cursor position
  replacing the old placement. If it double-images instead, delete the id first.
- 2v2 battle layout at various window sizes (needs ≥76×26; shows a
  "please enlarge" notice below that).
- Menu backdrop: on a narrow terminal the content-sized panel may crowd the two
  fighters. Fix by capping panel width or pushing the fighters further out.

## Known rough edges

- `scene_shadow_dragon` reads as an epic purple/gold vista rather than a literal
  treasure cavern. Re-roll if wanted (the other five are on-theme).
- Dead-code warnings, all intentional: `Stat::Atk` (no move touches Attack yet),
  `Battler.sprite` / `.is_player` (battle UI loads art from the item/enemy
  instead), `MenuItem::disabled`.
- Weapon splashes took three passes to get single, correctly-typed objects.
  If adding a weapon, expect to check the result and re-roll.
- `git status` is dirty — the latest work is uncommitted.

## Possible next steps

Nothing is blocked; these are ideas, roughly by value:

1. **Play it in Ghostty** and tune timings (all pacing knobs are listed at the
   bottom of `CLAUDE.md`).
2. **Weapon pixel sprites** — weapons currently fall back to a downscaled splash
   in the collection footer; they could get proper drawn sprites.
3. **Equip weapons in battle** — weapons are collectible but have no combat
   effect yet (an ATK/DEF bonus would make `Stat::Atk` live).
4. **More enemies / an endless or boss-rush mode**, reusing the scene pipeline.
5. **Character levels or constellations affecting battle stats** — duplicates
   currently only increment a counter.
6. **Weapon banner** with Genshin's Epitomized Path.

## Rebuild-from-scratch notes

Art is committed, so a fresh clone needs only Rust. To regenerate art you need
the `artgen` conda env (see `CLAUDE.md`) and roughly 3–6 min of GPU per image.

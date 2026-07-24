#!/usr/bin/env python3
"""Generate character/weapon portraits with a local anime SDXL model.

Reads prompt data exported from the game (`wish --export-prompts`) and writes
one PNG per item into assets/portraits/. Runs on Apple Silicon via MPS.

Usage:
    python scripts/generate_art.py                 # all items
    python scripts/generate_art.py yukihana guren  # only these ids
"""
import os
# Let MPS spill to shared memory instead of hard-failing on this 16GB machine.
os.environ.setdefault("PYTORCH_MPS_HIGH_WATERMARK_RATIO", "0.0")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

import json
import sys
import time
from pathlib import Path

import torch
from diffusers import StableDiffusionXLPipeline, EulerAncestralDiscreteScheduler

MODEL = "cagliostrolab/animagine-xl-4.0"
ROOT = Path(__file__).resolve().parent.parent
PROMPTS = ROOT / "prompts.json"
OUT = ROOT / "assets" / "portraits"

WIDTH, HEIGHT = 832, 1216
STEPS = int(os.environ.get("GEN_STEPS", "26"))
GUIDANCE = 6.0
# Attention slicing is safe on 16GB but ~2x slower; set GEN_SLICE=0 to disable.
SLICE = os.environ.get("GEN_SLICE", "1") != "0"

QUALITY = "masterpiece, high score, great quality, absurdres, very aesthetic"
NEG_BASE = (
    "lowres, worst quality, low quality, bad quality, jpeg artifacts, bad anatomy, "
    "bad hands, bad proportions, extra digits, fewer digits, extra limbs, "
    "watermark, signature, username, text, logo, error, blurry, nsfw, nude, "
    "explicit, revealing clothes"
)
# Weapons must be exactly ONE object — SDXL loves to make variation sheets, so
# the plural terms are heavily negated and the framing avoids "concept art".
NEG_WEAPON = (
    NEG_BASE
    + ", 1girl, 1boy, person, human, face, hands, character, "
    + "multiple weapons, two weapons, three weapons, four weapons, many weapons, "
    + "row of weapons, weapons in a row, side by side, lineup, collection, set, "
    + "variations, reference sheet, design sheet, chart, grid, pair, duplicate, "
    + "group, several objects, weapon rack, multiple views, "
    + "arrow through the bow, arrow passing through bow, arrow in center of bow, "
    + "bow without string, stringless bow, multiple arrows"
)


def seed_for(item_id: str) -> int:
    return abs(hash(item_id)) % (2**31)


def build(item):
    # Keep prompts within CLIP's 77-token window; `look` carries the identity,
    # so lead with short framing tags and let `look` follow.
    look = item["look"]
    if item["kind"] == "character":
        prompt = f"{QUALITY}, safe, anime splash art, cinematic lighting, {look}"
        return prompt, NEG_BASE
    # Weapon: force exactly ONE item of the CORRECT type. Lead with an explicit
    # shape hint (the type name alone renders bows as sticks), then the look.
    shape = {
        "Sword": "a single straight one-handed sword with a cross-guard hilt",
        "Claymore": "a single massive two-handed greatsword, very large wide heavy blade",
        "Polearm": "a single long polearm spear, bladed tip on a long shaft",
        "Bow": (
            "a single curved archery bow held vertically with a taut visible "
            "bowstring, one arrow nocked and resting against the side of the bow, "
            "arrow off to the side not passing through the bow, clear recurve bow shape"
        ),
        "Catalyst": "a single floating magical catalyst",
    }.get(item.get("weapon", ""), f"a single {item.get('weapon','weapon')}")
    prompt = (
        f"solo, {shape}, only one weapon, one object, centered, "
        f"isolated on plain dark gradient background, no humans, {QUALITY}, {look}"
    )
    return prompt, NEG_WEAPON


SCENE_NEG = (
    "people, person, human, girl, boy, character, creature, monster, portrait, "
    "text, watermark, signature, blurry, lowres, ui, hud"
)


def main():
    items = json.loads(PROMPTS.read_text())
    # Enemies get pixel sprites only (generate_pixel.py), not splashes.
    items = [i for i in items if i["kind"] != "enemy"]
    wanted = set(sys.argv[1:])
    if wanted:
        items = [i for i in items if i["id"] in wanted]
    OUT.mkdir(parents=True, exist_ok=True)

    print(f"Loading {MODEL} (first run downloads ~7GB)…", flush=True)
    t0 = time.time()
    pipe = StableDiffusionXLPipeline.from_pretrained(
        MODEL, torch_dtype=torch.float16, use_safetensors=True, add_watermarker=False
    )
    pipe.scheduler = EulerAncestralDiscreteScheduler.from_config(pipe.scheduler.config)
    pipe.to("mps")
    if SLICE:
        pipe.enable_attention_slicing()
    pipe.enable_vae_tiling()
    pipe.set_progress_bar_config(disable=True)
    print(f"Model ready in {time.time() - t0:.0f}s. Generating {len(items)} images.", flush=True)

    scenes_dir = ROOT / "assets" / "scenes"
    for n, item in enumerate(items, 1):
        is_scene = item["kind"] == "scene"
        if is_scene:
            # Landscape backdrop, no characters at all.
            prompt = (
                f"fantasy game battle background, wide scenic landscape, {item['look']}, "
                f"atmospheric lighting, detailed environment art, no humans, no characters"
            )
            neg = SCENE_NEG
            w, h = 1216, 832
            out_dir = scenes_dir
        else:
            prompt, neg = build(item)
            w, h = WIDTH, HEIGHT
            out_dir = OUT
        out_dir.mkdir(parents=True, exist_ok=True)
        gen = torch.Generator(device="cpu").manual_seed(seed_for(item["id"]))
        t = time.time()
        image = pipe(
            prompt=prompt,
            negative_prompt=neg,
            width=w,
            height=h,
            num_inference_steps=STEPS,
            guidance_scale=GUIDANCE,
            generator=gen,
        ).images[0]
        path = out_dir / f"{item['id']}.png"
        image.save(path)
        print(f"[{n}/{len(items)}] {item['id']:<20} {time.time() - t:5.1f}s -> {path}", flush=True)

    print("Done.", flush=True)


if __name__ == "__main__":
    main()

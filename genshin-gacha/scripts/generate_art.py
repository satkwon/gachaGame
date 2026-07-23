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
NEG_WEAPON = NEG_BASE + ", 1girl, 1boy, person, human, face, hands"


def seed_for(item_id: str) -> int:
    return abs(hash(item_id)) % (2**31)


def build(item):
    # Keep prompts within CLIP's 77-token window; `look` carries the identity,
    # so lead with short quality/framing tags and let `look` follow.
    look = item["look"]
    if item["kind"] == "character":
        prompt = f"{QUALITY}, safe, anime splash art, cinematic lighting, {look}"
        return prompt, NEG_BASE
    prompt = f"{QUALITY}, item concept art, plain dark background, {look}, no humans"
    return prompt, NEG_WEAPON


def main():
    items = json.loads(PROMPTS.read_text())
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

    for n, item in enumerate(items, 1):
        prompt, neg = build(item)
        gen = torch.Generator(device="cpu").manual_seed(seed_for(item["id"]))
        t = time.time()
        image = pipe(
            prompt=prompt,
            negative_prompt=neg,
            width=WIDTH,
            height=HEIGHT,
            num_inference_steps=STEPS,
            guidance_scale=GUIDANCE,
            generator=gen,
        ).images[0]
        path = OUT / f"{item['id']}.png"
        image.save(path)
        print(f"[{n}/{len(items)}] {item['id']:<20} {time.time() - t:5.1f}s -> {path}", flush=True)

    print("Done.", flush=True)


if __name__ == "__main__":
    main()

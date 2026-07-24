#!/usr/bin/env python3
"""Generate detailed, arcade-style pixel-art character sprites.

Uses Animagine XL 4.0 with the pixel-art-xl LoRA, then cleans the output to a
crisp limited-palette sprite (downscale to a pixel grid + colour quantise +
nearest upscale). Writes assets/pixel/<id>.png, which the game prefers over the
hand-drawn fallback sprite.

Usage:
    python scripts/generate_pixel.py                # all characters
    python scripts/generate_pixel.py yukihana       # one
"""
import os
os.environ.setdefault("PYTORCH_MPS_HIGH_WATERMARK_RATIO", "0.0")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

import json
import sys
import time
from pathlib import Path

import torch
from diffusers import StableDiffusionXLPipeline, EulerAncestralDiscreteScheduler
from PIL import Image

MODEL = "cagliostrolab/animagine-xl-4.0"
LORA = "nerijs/pixel-art-xl"
LORA_FILE = "pixel-art-xl.safetensors"
LORA_SCALE = float(os.environ.get("PX_LORA", "0.9"))

ROOT = Path(__file__).resolve().parent.parent
PROMPTS = ROOT / "prompts.json"
OUT = ROOT / "assets" / "pixel"

SIZE = 1024
STEPS = int(os.environ.get("PX_STEPS", "30"))
GUIDANCE = float(os.environ.get("PX_CFG", "7.0"))
GRID_H = int(os.environ.get("PX_GRID", "144"))   # sprite pixel-grid height
COLORS = int(os.environ.get("PX_COLORS", "48"))  # palette size
UPSCALE = 4

# Pokemon-style sprites: cute chibi, clean thick outline, flat shading, and a
# STANDARDIZED full-body front-view framing. Framing/style tokens lead the
# prompt so CLIP's 77-token limit never truncates them; each character's `look`
# (written to match its splash) keeps the sprite inspired by the splash art.
NEG = (
    "close-up, extreme close-up, cropped, headshot, bust shot, zoomed in, "
    "portrait crop, out of frame, 3d, blurry, soft, smooth gradient shading, "
    "realistic, photo, jpeg artifacts, lowres, bad anatomy, deformed, extra limbs, "
    "detailed background, scenery, complex background, watermark, signature, text, "
    "nsfw, nude"
)


def build(item):
    if item["kind"] == "enemy":
        # A SINGLE classic-RPG monster. The pixel LoRA loves sprite sheets, so
        # lead hard with singularity tags and heavily negate any grid/sheet.
        prompt = (
            f"solo, 1other, a single monster, only one creature, one monster, "
            f"pokemon style creature, chibi pixel art, thick black outline, "
            f"flat cel shading, full body, front view, centered, "
            f"{item['look']}, simple flat background"
        )
        neg = (
            NEG.replace("nsfw, nude", "nsfw")
            + ", sprite sheet, reference sheet, character sheet, multiple monsters, "
            + "many creatures, two monsters, several, grid, tiles, rows, columns, "
            + "collection, set, variations, duplicate, side by side, group, lineup, "
            + "human, 1girl, girl, person"
        )
        return prompt, neg
    prompt = (
        f"pokemon game sprite, safe, cute chibi pixel art, thick black outline, "
        f"flat cel shading, full body, standing, front view, centered, "
        f"{item['look']}, simple flat pastel background"
    )
    return prompt, NEG


def pixelate(img: Image.Image) -> Image.Image:
    """Crush to a pixel grid and lightly quantise for crisp, colourful pixels."""
    w, h = img.size
    gw = max(1, round(GRID_H * w / h))
    small = img.convert("RGB").resize((gw, GRID_H), Image.Resampling.LANCZOS)
    # Octree keeps hues faithful; per-image palette avoids a global colour cast.
    quant = small.quantize(colors=COLORS, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE)
    flat = quant.convert("RGB")
    return flat.resize((gw * UPSCALE, GRID_H * UPSCALE), Image.Resampling.NEAREST)


def main():
    items = [i for i in json.loads(PROMPTS.read_text()) if i["kind"] in ("character", "enemy")]
    wanted = set(sys.argv[1:])
    if wanted:
        items = [i for i in items if i["id"] in wanted]
    OUT.mkdir(parents=True, exist_ok=True)

    print(f"Loading {MODEL} + {LORA} …", flush=True)
    t0 = time.time()
    pipe = StableDiffusionXLPipeline.from_pretrained(
        MODEL, torch_dtype=torch.float16, use_safetensors=True, add_watermarker=False
    )
    pipe.scheduler = EulerAncestralDiscreteScheduler.from_config(pipe.scheduler.config)
    pipe.load_lora_weights(LORA, weight_name=LORA_FILE)
    # Do NOT fuse: fusing in fp16 corrupts colours here. Apply the LoRA at
    # inference via cross_attention scale instead.
    pipe.to("mps")
    pipe.vae.enable_tiling()
    pipe.set_progress_bar_config(disable=True)
    print(f"Ready in {time.time() - t0:.0f}s. Generating {len(items)} sprites.", flush=True)

    for n, item in enumerate(items, 1):
        prompt, neg = build(item)
        gen = torch.Generator(device="cpu").manual_seed(abs(hash(item["id"])) % (2**31))
        t = time.time()
        img = pipe(
            prompt=prompt,
            negative_prompt=neg,
            width=SIZE,
            height=SIZE,
            num_inference_steps=STEPS,
            guidance_scale=GUIDANCE,
            generator=gen,
            cross_attention_kwargs={"scale": LORA_SCALE},
        ).images[0]
        if os.environ.get("PX_RAW"):
            img.save(OUT / f"_raw_{item['id']}.png")
        sprite = pixelate(img)
        path = OUT / f"{item['id']}.png"
        sprite.save(path)
        print(f"[{n}/{len(items)}] {item['id']:<14} {time.time() - t:5.1f}s -> {path}", flush=True)

    print("Done.", flush=True)


if __name__ == "__main__":
    main()

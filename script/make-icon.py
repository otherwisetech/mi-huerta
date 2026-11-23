#!/usr/bin/env python3
"""Draws Mi Huerta's icon (a green rounded square with a cream sprout and a little tomato) and
writes every size the app and the bundles need. Needs Pillow; iconutil (macOS) for the .icns.

Usage: python3 script/make-icon.py
Writes crates/app/assets/logo/{mi-huerta-icon-1024.png, -512.png, -256.png, AppIcon.icns,
AppIcon.ico, mi-huerta-mark.svg, mi-huerta-icon.svg}.
"""
import pathlib, shutil, subprocess, tempfile
from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/app/assets/logo"
S = 2048  # drawn at 2x, then downsampled
TOP, BOTTOM = (0x4C, 0xA8, 0x55), (0x1F, 0x63, 0x2C)
CREAM, LEAF_DARK = (0xF6, 0xF1, 0xE3), (0xDD, 0xEB, 0xC9)
TOMATO, TOMATO_HI = (0xE0, 0x4A, 0x36), (0xF2, 0x7B, 0x5E)
SOIL = (0x6B, 0x45, 0x2A)


def k(v):
    return int(v * S / 1024)


def draw():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    # background: vertical gradient inside a rounded square (macOS-style margins)
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        t = min(max((y - k(100)) / k(824), 0), 1)
        gd.line([(0, y), (S, y)], fill=tuple(int(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3)) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([k(100), k(100), k(924), k(924)], radius=k(185), fill=255)
    img.paste(grad, (0, 0), mask)
    d = ImageDraw.Draw(img)
    # soil mound
    d.chord([k(250), k(700), k(774), k(900)], 180, 360, fill=SOIL + (255,))
    # stem
    d.rounded_rectangle([k(492), k(400), k(532), k(800)], radius=k(20), fill=CREAM + (255,))
    # leaves: two ellipses rotated
    for (cx, cy, ang, col) in [(k(392), k(430), 35, CREAM), (k(636), k(360), -35, LEAF_DARK)]:
        leaf = Image.new("RGBA", (k(320), k(170)), (0, 0, 0, 0))
        ImageDraw.Draw(leaf).ellipse([0, 0, k(320), k(170)], fill=col + (255,))
        leaf = leaf.rotate(ang, expand=True, resample=Image.BICUBIC)
        img.alpha_composite(leaf, (cx - leaf.width // 2, cy - leaf.height // 2))
    # a tomato on the soil
    d = ImageDraw.Draw(img)
    d.ellipse([k(600), k(600), k(760), k(760)], fill=TOMATO + (255,))
    d.ellipse([k(630), k(630), k(680), k(670)], fill=TOMATO_HI + (255,))
    d.polygon([(k(680), k(590)), (k(700), k(615)), (k(720), k(592)), (k(706), k(625)), (k(680), k(628))], fill=CREAM + (255,))
    return img


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    big = draw()
    sizes = {s: big.resize((s, s), Image.LANCZOS) for s in [16, 32, 48, 64, 128, 256, 512, 1024]}
    for s in (1024, 512, 256):
        sizes[s].save(OUT / f"mi-huerta-icon-{s}.png")
    sizes[256].save(OUT / "AppIcon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    if shutil.which("iconutil"):
        with tempfile.TemporaryDirectory() as t:
            iconset = pathlib.Path(t) / "AppIcon.iconset"
            iconset.mkdir()
            for s in [16, 32, 128, 256, 512]:
                big.resize((s, s), Image.LANCZOS).save(iconset / f"icon_{s}x{s}.png")
                big.resize((s * 2, s * 2), Image.LANCZOS).save(iconset / f"icon_{s}x{s}@2x.png")
            subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(OUT / "AppIcon.icns")], check=True)
    # the one-colour in-app mark (tinted by the app): a sprout
    (OUT / "mi-huerta-mark.svg").write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
        '<path d="M12 21v-9"/><path d="M12 12c0-4-3-6-7-6 0 4 3 6 7 6z" fill="currentColor"/><path d="M12 10c0-3.5 2.5-5.5 7-5.5 0 3.5-2.5 5.5-7 5.5z" fill="currentColor"/><path d="M6 21h12"/></svg>\n'
    )
    print("icons written to", OUT)


if __name__ == "__main__":
    main()

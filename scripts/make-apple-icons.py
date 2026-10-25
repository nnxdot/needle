"""The Mac's, the iPhone's, and the watch's app icons, from Needle's icon (assets/icon, drawn by
scripts/make-icon.py), as PNG files the builds use:

- apps/macos/Needle.iconset: every size macOS asks for, the icon as drawn (its squircle with
  clear corners); 16 to 64 px from needle-small.svg (no grooves).
- the iPhone's and the watch's AppIcon (1024 px): the same drawing on its charcoal filling the
  whole square, as iOS and watchOS cut their own shape (clear corners would show black).

Needs resvg (pip install resvg-py). Run it again after make-icon.py:

    python scripts/make-apple-icons.py
"""
import io
from pathlib import Path

import resvg_py
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "assets" / "icon"


def render(svg: str, size: int) -> Image.Image:
    data = bytes(resvg_py.svg_to_bytes(svg_string=svg, width=size, height=size))
    return Image.open(io.BytesIO(data)).convert("RGBA")


def full_square(svg: str) -> str:
    """The icon's charcoal over the whole square: no rounded corners, no rim."""
    plate = '<rect width="512" height="512" rx="115" fill="url(#bg)"/>'
    rim = '<rect x="2" y="2" width="508" height="508" rx="113" fill="none" stroke="#fff" stroke-opacity=".09" stroke-width="3"/>'
    assert plate in svg and rim in svg, "the icon's plate changed: update make-apple-icons.py"
    return svg.replace(plate, '<rect width="512" height="512" fill="url(#bg)"/>').replace(rim, "")


full = (ICON / "needle.svg").read_text(encoding="utf-8")
small = (ICON / "needle-small.svg").read_text(encoding="utf-8")

# The Mac: an iconset for iconutil (build-mac-app.sh).
iconset = ROOT / "apps" / "macos" / "Needle.iconset"
iconset.mkdir(parents=True, exist_ok=True)
for size in (16, 32, 128, 256, 512):
    for scale in (1, 2):
        pixels = size * scale
        name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
        render(small if pixels <= 64 else full, pixels).save(iconset / name, optimize=True)

# The iPhone and the watch: one 1024 px picture each, with no clear parts.
square = render(full_square(full), 1024)
for target in (
    ROOT / "apps" / "ios" / "Needle" / "Assets.xcassets" / "AppIcon.appiconset" / "icon-1024.png",
    ROOT / "apps" / "ios" / "NeedleWatch" / "Assets.xcassets" / "AppIcon.appiconset" / "icon-1024.png",
):
    background = Image.new("RGB", square.size, (20, 18, 17))
    background.paste(square, mask=square.split()[3])
    background.save(target, optimize=True)
print("made the Mac iconset and the iPhone's and watch's icons")

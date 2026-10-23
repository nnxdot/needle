"""Made-up artist pictures for the test library: a figure in light and colour, drawn here."""
import blake3
from PIL import Image, ImageDraw, ImageFilter

W, H = 1000, 1100
# python artists.py <data folder>: the pictures go where Needle keeps artist photos.
import os, sys
OUT = os.path.join(sys.argv[1], 'artwork', 'artists') + os.sep
os.makedirs(OUT, exist_ok=True)

artists = {
    'Mara Quinn': ((40, 20, 10), (242, 162, 94), (255, 220, 180)),
    'Pixel Choir': ((40, 10, 40), (236, 106, 134), (255, 200, 230)),
    'The Pressings': ((25, 25, 30), (201, 201, 207), (255, 255, 255)),
    'Hana Mori': ((60, 20, 20), (242, 141, 108), (255, 220, 200)),
    'Juniper Bay': ((50, 10, 20), (232, 80, 94), (255, 210, 200)),
    'Fern & Field': ((25, 35, 15), (201, 176, 100), (240, 230, 190)),
    'Low Ceiling': ((15, 25, 35), (127, 164, 192), (220, 235, 245)),
    'Darkroom': ((35, 15, 5), (232, 160, 82), (255, 220, 170)),
    'Oda Lind': ((20, 15, 45), (143, 120, 232), (220, 210, 255)),
    'Sol Arden': ((45, 20, 5), (224, 136, 72), (255, 215, 170)),
}


def gradient(top, bottom):
    img = Image.new('RGB', (W, H))
    d = ImageDraw.Draw(img)
    for y in range(H):
        t = y / (H - 1)
        d.line([(0, y), (W, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)))
    return img


def figure(dx, scale):
    """A head and shoulders, as a mask."""
    mask = Image.new('L', (W, H), 0)
    d = ImageDraw.Draw(mask)
    cx = W // 2 + dx
    hw, hh = int(150 * scale), int(190 * scale)
    top = int(H * 0.30)
    d.ellipse((cx - hw, top, cx + hw, top + 2 * hh), fill=255)
    d.rectangle((cx - int(70 * scale), top + int(1.7 * hh), cx + int(70 * scale), top + int(2.4 * hh)), fill=255)
    d.rounded_rectangle((cx - int(380 * scale), top + int(2.25 * hh), cx + int(380 * scale), H + 200), radius=int(260 * scale), fill=255)
    return mask.filter(ImageFilter.GaussianBlur(3))


for i, (name, (bg, glow, rim)) in enumerate(artists.items()):
    img = gradient(bg, tuple(int(c * 0.55) for c in bg)).convert('RGBA')
    halo = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(halo).ellipse((W * 0.1, H * 0.05, W * 0.9, H * 0.85), fill=glow + (190,))
    img.alpha_composite(halo.filter(ImageFilter.GaussianBlur(120)))
    groups = [(0, 1.0)] if i % 2 == 0 else [(-170, 0.82), (170, 0.82)]
    for dx, scale in groups:
        m = figure(dx, scale)
        # The edge lit from behind, then the figure in shadow.
        rim_layer = Image.new('RGBA', (W, H), rim + (0,))
        rim_layer.putalpha(m.filter(ImageFilter.GaussianBlur(14)).point(lambda v: int(v * 0.9)))
        img.alpha_composite(rim_layer)
        body = Image.new('RGBA', (W, H), tuple(int(c * 0.25) for c in bg) + (0,))
        body.putalpha(m.filter(ImageFilter.GaussianBlur(2)).point(lambda v: int(v * 0.97)).transform((W, H), Image.AFFINE, (1, 0, -6, 0, 1, -6)))
        img.alpha_composite(body)
    grain = Image.effect_noise((W, H), 30).convert('L').point(lambda v: 22 if v > 150 else 0)
    g = Image.new('RGBA', (W, H), (255, 255, 255, 0))
    g.putalpha(grain)
    img.alpha_composite(g)
    file = blake3.blake3(name.lower().encode()).hexdigest() + '.img'
    img.convert('RGB').save(OUT + file, 'JPEG', quality=90)
    print(name, file)

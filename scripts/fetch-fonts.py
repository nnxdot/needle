"""Needle's fonts (all SIL Open Font License), fetched from Google Fonts as plain TTF files into
assets/fonts, with each family's license. Every app takes its fonts from that folder.

python scripts/fetch-fonts.py"""
import re
import urllib.request
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets" / "fonts"

# Google Fonts family, the files' name, weights, extra axes ("" for none), and the license's
# folder in github.com/google/fonts.
FONTS = [
    ("Roboto Condensed", "Poster", [800, 900], "", "robotocondensed"),
    ("Instrument Serif", "InstrumentSerif", [400], "", "instrumentserif"),
    ("DM Serif Display", "DMSerifDisplay", [400], "", "dmserifdisplay"),
    ("Space Grotesk", "SpaceGrotesk", [600, 700], "", "spacegrotesk"),
    ("Bricolage Grotesque", "BricolageGrotesque", [600, 800], "", "bricolagegrotesque"),
    ("Outfit", "Outfit", [600, 700], "", "outfit"),
    ("Inter", "Inter", [400, 500, 600, 700], "", "inter"),
    ("Atkinson Hyperlegible", "AtkinsonHyperlegible", [400, 700], "", "atkinsonhyperlegible"),
    ("Lexend", "Lexend", [400, 500, 600, 700], "", "lexend"),
    ("IBM Plex Mono", "IBMPlexMono", [400, 700], "", "ibmplexmono"),
]


def get(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "needle-fonts"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


OUT.mkdir(parents=True, exist_ok=True)
for family, name, weights, axes, license_dir in FONTS:
    for weight in weights:
        query = family.replace(" ", "+")
        spec = f"{axes}{weight}" if axes else f"wght@{weight}"
        css = get(f"https://fonts.googleapis.com/css2?family={query}:{spec}").decode()
        url = re.search(r"url\((https://[^)]+\.ttf)\)", css).group(1)
        file = OUT / f"{name}-{weight}.ttf"
        file.write_bytes(get(url))
        print(file.name, file.stat().st_size)
    license_text = None
    for folder in ("ofl", "apache"):
        for leaf in ("OFL.txt", "LICENSE.txt"):
            try:
                license_text = get(f"https://raw.githubusercontent.com/google/fonts/main/{folder}/{license_dir}/{leaf}")
                break
            except Exception:
                continue
        if license_text:
            break
    assert license_text, f"no license for {family}"
    (OUT / f"{name}-OFL.txt").write_bytes(license_text)

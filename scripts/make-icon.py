"""Draws Needle 2.0's icon, "Glass groove": the amber record as glass with a soft amber glow
behind it, the cream tonearm, on a charcoal squircle. Writes assets/icon/needle.svg (the app
icon), assets/icon/needle-small.svg (no grooves, for 16-64 px), and assets/icon/needle-mark.svg
(the record and arm alone, for places that draw their own background).

    python scripts/make-icon.py
"""
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "assets" / "icon"

SHADOW = (
    '<filter id="shadow" x="-30%" y="-30%" width="160%" height="160%">'
    '<feDropShadow dx="0" dy="8" stdDeviation="10" flood-color="#000" flood-opacity=".35"/></filter>'
)


def plate():
    return (
        '<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">'
        '<stop offset="0" stop-color="#2d2926"/><stop offset="1" stop-color="#141211"/></linearGradient>',
        '<rect width="512" height="512" rx="115" fill="url(#bg)"/>'
        '<rect x="2" y="2" width="508" height="508" rx="113" fill="none" stroke="#fff" stroke-opacity=".09" stroke-width="3"/>',
    )


def disc(cx, cy, r, grooves):
    defs = (
        '<radialGradient id="disc" cx=".36" cy=".3" r=".85"><stop offset="0" stop-color="#f6d9a4"/>'
        '<stop offset=".55" stop-color="#e2b46c"/><stop offset="1" stop-color="#ad7a3c"/></radialGradient>'
        '<linearGradient id="shine" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".6"/>'
        '<stop offset=".42" stop-color="#fff" stop-opacity="0"/></linearGradient>'
        '<radialGradient id="glow" cx=".5" cy=".5" r=".5"><stop offset="0" stop-color="#e2b46c" stop-opacity=".5"/>'
        '<stop offset="1" stop-color="#e2b46c" stop-opacity="0"/></radialGradient>'
    )
    body = f'<circle cx="{cx}" cy="{cy}" r="{r * 1.45:g}" fill="url(#glow)"/><circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#disc)"/>'
    if grooves:
        for k in (0.86, 0.74, 0.62):
            body += f'<circle cx="{cx}" cy="{cy}" r="{r * k:g}" fill="none" stroke="#7a5020" stroke-opacity=".35" stroke-width="{r * 0.02:g}"/>'
    body += (
        f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#shine)" opacity=".55"/>'
        f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="#fff" stroke-opacity=".25" stroke-width="2"/>'
        f'<circle cx="{cx}" cy="{cy}" r="{r * 0.36:g}" fill="#1b1816"/><circle cx="{cx}" cy="{cy}" r="{r * 0.08:g}" fill="#e2b46c"/>'
    )
    return defs, body


def arm(cx, cy):
    # The pivot at the top right; the arm comes down to the record's edge.
    px, py, ex, ey = cx + 184, cy - 190, cx + 82, cy - 64
    return (
        f'<g filter="url(#shadow)"><line x1="{px}" y1="{py}" x2="{ex}" y2="{ey}" stroke="#f2e8d6" stroke-width="22" stroke-linecap="round"/>'
        f'<circle cx="{px}" cy="{py}" r="30" fill="#f2e8d6"/><circle cx="{px}" cy="{py}" r="12" fill="#1b1816"/></g>'
    )


def icon(with_plate=True, grooves=True):
    defs, body = disc(236, 288, 150, grooves)
    plate_defs, plate_body = plate() if with_plate else ("", "")
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">'
        f"<defs>{SHADOW}{plate_defs}{defs}</defs>{plate_body}{body}{arm(236, 288)}</svg>\n"
    )


OUT.mkdir(parents=True, exist_ok=True)
(OUT / "needle.svg").write_bytes(icon().encode())
(OUT / "needle-small.svg").write_bytes(icon(grooves=False).encode())
(OUT / "needle-mark.svg").write_bytes(icon(with_plate=False).encode())
print(f"Wrote the icons in {OUT}")

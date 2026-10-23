"""A made-up library for the website's pictures: the site's own made-up albums (covers are
free Unsplash photos, website/site/public/img/covers), as silent 24-bit / 48 kHz FLAC files
with real tags, a cover.jpg in each album folder, and timed lyrics for "Opening".

Writes the FLAC files by hand (CONSTANT subframes of silence), so they are a few KB each."""
import os, struct, sys
from PIL import Image

# python library.py <music folder to make> <repo>/website/site/public/img/covers
ROOT, COVERS = sys.argv[1], sys.argv[2]
RATE, BITS, CHANNELS, BLOCK = 48000, 24, 2, 4096

# album id, title, artist, year, genre, songs (title, length "m:ss")
ALBUMS = [
    ('harbor-lights', 'Harbor Lights', 'Mara Quinn', 2021, 'Indie Pop',
     [('Opening', '3:48'), ('Harbor Lights', '4:05'), ('Salt and Static', '3:31'), ('Low Tide', '4:22')]),
    ('tidewater', 'Tidewater', 'Mara Quinn', 2024, 'Indie Pop',
     [('Tidewater', '3:56'), ('Undertow', '4:11'), ('Breakwater', '3:39')]),
    ('middle-distance', 'Middle Distance', 'Mara Quinn', 2024, 'Indie Pop',
     [('Middle Distance', '4:01'), ('Far Shore', '3:44'), ('Glass Weather', '3:58')]),
    ('afterglow-arcade', 'Afterglow Arcade', 'Pixel Choir', 2023, 'Synthpop',
     [('Coming Home', '3:27'), ('Neon Rain', '3:52'), ('High Score', '3:15'), ('Last Credit', '4:30')]),
    ('single-needle', 'Single Needle', 'The Pressings', 1998, 'Rock',
     [('Single Needle', '3:20'), ('B-Side', '2:58'), ('Run-Out Groove', '4:07')]),
    ('bloom-season', 'Bloom Season', 'Hana Mori', 2025, 'Dream Pop',
     [('Paper Lanterns', '4:12'), ('First Light', '3:35'), ('Garden Hours', '4:03')]),
    ('berry-season', 'Berry Season', 'Juniper Bay', 2022, 'Indie Folk',
     [('Berry Season', '3:41'), ('Long Summer', '4:16'), ('Picking Rows', '3:09')]),
    ('understory', 'Understory', 'Fern & Field', 2019, 'Folk',
     [('Understory', '4:25'), ('Moss and Stone', '3:50'), ('Canopy', '3:37')]),
    ('grey-weather', 'Grey Weather', 'Low Ceiling', 2017, 'Shoegaze',
     [('Overcast', '4:48'), ('Drizzle', '3:55'), ('Pressure Drop', '5:12')]),
    ('red-light-only', 'Red Light Only', 'Darkroom', 2020, 'Post-Punk',
     [('Developer', '3:26'), ('Middle Distance', '3:59'), ('Fixer', '3:44')]),
    ('night-ferry', 'Night Ferry', 'Oda Lind', 2024, 'Ambient Pop',
     [('Last Crossing', '5:03'), ('Starboard', '4:18'), ('Harbour Bell', '3:47')]),
    ('low-desert', 'Low Desert', 'Sol Arden', 2018, 'Americana',
     [('Low Desert', '4:09'), ('Heat Mirage', '3:33'), ('Night Drive West', '4:41')]),
]

LYRICS = """[00:12.00]Harbor lights are coming on
[00:17.50]The tide is turning slow
[00:23.00]Every song I ever kept
[00:28.50]Is right where I left it
[00:34.00]Put the needle down again
[00:39.50]And let the night go on
[00:46.00]
[01:02.00]Harbor lights are coming on
[01:07.50]The tide is turning slow
"""


def crc8(data):
    crc = 0
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0x07) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def crc16(data):
    crc = 0
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x8005) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def utf8_number(n):
    if n < 0x80:
        return bytes([n])
    if n < 0x800:
        return bytes([0xC0 | (n >> 6), 0x80 | (n & 0x3F)])
    return bytes([0xE0 | (n >> 12), 0x80 | ((n >> 6) & 0x3F), 0x80 | (n & 0x3F)])


def frame(number, samples):
    # Block size: 4096 by code, or a last, shorter block as 16 bits after the header.
    code = 0b1100 if samples == BLOCK else 0b0111
    head = bytes([0xFF, 0xF8, (code << 4) | 0b1010, (0b0001 << 4) | (0b110 << 1)])
    head += utf8_number(number)
    if code == 0b0111:
        head += struct.pack('>H', samples - 1)
    head += bytes([crc8(head)])
    # Two CONSTANT subframes of 0: a header byte (type 000000) and a 24-bit value each.
    body = head + bytes([0, 0, 0, 0]) * CHANNELS
    return body + struct.pack('>H', crc16(body))


def flac(path, tags, seconds):
    total = int(seconds * RATE)
    info = struct.pack('>HH', BLOCK, BLOCK) + (0).to_bytes(3, 'big') + (0).to_bytes(3, 'big')
    packed = (RATE << 44) | ((CHANNELS - 1) << 41) | ((BITS - 1) << 36) | total
    info += packed.to_bytes(8, 'big') + bytes(16)
    vendor = b'needle-site-pictures'
    comments = [f'{k}={v}'.encode('utf-8') for k, v in tags.items()]
    vc = struct.pack('<I', len(vendor)) + vendor + struct.pack('<I', len(comments))
    vc += b''.join(struct.pack('<I', len(c)) + c for c in comments)
    out = bytearray(b'fLaC')
    out += bytes([0]) + len(info).to_bytes(3, 'big') + info
    out += bytes([0x80 | 4]) + len(vc).to_bytes(3, 'big') + vc
    n, left = 0, total
    while left > 0:
        samples = min(BLOCK, left)
        out += frame(n, samples)
        left -= samples
        n += 1
    with open(path, 'wb') as f:
        f.write(out)


def seconds(text):
    m, s = text.split(':')
    return int(m) * 60 + int(s)


for album_id, album, artist, year, genre, songs in ALBUMS:
    folder = os.path.join(ROOT, artist, album)
    os.makedirs(folder, exist_ok=True)
    Image.open(os.path.join(COVERS, album_id + '.webp')).convert('RGB').resize((1000, 1000)).save(
        os.path.join(folder, 'cover.jpg'), quality=92)
    for n, (title, length) in enumerate(songs, 1):
        name = f'{n:02d} {title}'
        flac(os.path.join(folder, name + '.flac'), {
            'TITLE': title, 'ARTIST': artist, 'ALBUM': album, 'ALBUMARTIST': artist,
            'DATE': str(year), 'GENRE': genre, 'TRACKNUMBER': str(n), 'TRACKTOTAL': str(len(songs)),
        }, seconds(length))
        if title == 'Opening':
            with open(os.path.join(folder, name + '.lrc'), 'w', encoding='utf-8') as f:
                f.write(LYRICS)
print('albums', len(ALBUMS), 'songs', sum(len(a[5]) for a in ALBUMS))

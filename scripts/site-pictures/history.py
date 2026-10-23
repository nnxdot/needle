"""Made-up listening history, ratings, and play counts for the website's test library."""
import json, random, sqlite3, sys, time, uuid

# python history.py <data folder>/library.db, after Needle has scanned the music folder.
db = sys.argv[1]
c = sqlite3.connect(db)
random.seed(7)
tracks = c.execute('select id, title, artist, album, duration from tracks').fetchall()
# Some favourites play more often.
weight = {t[0]: random.choice([1, 1, 2, 3, 6]) for t in tracks}
for name in ['Opening', 'Paper Lanterns', 'Last Crossing', 'Coming Home', 'Overcast', 'Middle Distance']:
    for t in tracks:
        if t[1] == name:
            weight[t[0]] = 12
now = int(time.time())
listens = []
for _ in range(795):
    t = random.choices(tracks, weights=[weight[x[0]] for x in tracks])[0]
    day = now - random.randint(0, 360) * 86400
    # Mostly late in the evening (around 11 PM local), some through the day.
    hour = random.choices([23, 22, 21, 0, 18, 9, 13], weights=[30, 18, 12, 10, 10, 8, 12])[0]
    local = time.localtime(day)
    started = int(time.mktime((local.tm_year, local.tm_mon, local.tm_mday, hour,
                               random.randint(0, 59), 0, 0, 0, -1)))
    if started > now:
        started -= 86400
    lid = uuid.uuid4().hex
    data = {'id': lid, 'track_id': t[0], 'title': t[1], 'artist': t[2], 'album': t[3],
            'started_at': started, 'listened_seconds': t[4], 'duration': t[4], 'qualified': True}
    listens.append((lid, t[0], started, 1, json.dumps(data)))
c.execute('delete from listens')
c.executemany('insert into listens (id, track_id, started_at, qualified, data) values (?,?,?,?,?)', listens)
for tid, *_ in tracks:
    plays = [l for l in listens if l[1] == tid]
    count, last = len(plays), max((l[2] for l in plays), default=0)
    rating = 5 if weight[tid] >= 12 else (4 if weight[tid] == 6 else 0)
    data = json.loads(c.execute('select data from tracks where id=?', (tid,)).fetchone()[0])
    data.update(play_count=count, last_played=last or None, rating=rating)
    c.execute('update tracks set play_count=?, last_played=?, rating=?, data=? where id=?',
              (count, last or None, rating, json.dumps(data, ensure_ascii=False), tid))
c.commit()
hours = sum(l and json.loads(l[4])['listened_seconds'] for l in listens) / 3600
print('listens', len(listens), 'hours', round(hours))

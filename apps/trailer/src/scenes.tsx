import React from "react";
import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame } from "remotion";
import { AMBER, EASE, Headline, INK, MUTED, Phone, Scene, useRise } from "./ui";

// ---------- 1. Opening: the record, then the line.

export const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const icon = useRise(4, 30);
  return (
    <Scene>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", gap: 48 }}>
        <Img
          src={staticFile("icon.png")}
          style={{
            width: 220,
            height: 220,
            borderRadius: 52,
            opacity: icon,
            scale: interpolate(icon, [0, 1], [0.8, 1]),
            rotate: `${interpolate(frame, [0, 90], [-30, 0], { extrapolateRight: "clamp", easing: EASE })}deg`,
          }}
        />
        <Headline text="Your music." start={26} size={110} />
        <Headline text="Now in your pocket." start={44} size={110} color={MUTED} style={{ marginTop: -32 }} />
      </AbsoluteFill>
    </Scene>
  );
};

// ---------- 2. Colour: album pages in their cover's colours.

export const Colour: React.FC = () => {
  const frame = useCurrentFrame();
  const phones = ["album", "album2", "artist"];
  return (
    <Scene>
      <AbsoluteFill style={{ padding: "110px 140px", flexDirection: "row", alignItems: "center", gap: 90 }}>
        <div style={{ width: 620 }}>
          <Headline text="Every album," start={0} size={96} />
          <Headline text="in its own colours." start={10} size={96} color={AMBER} />
          <div style={{ height: 36 }} />
          <Headline
            text="Pages take their colour from the cover, with the art drifting softly behind."
            start={24}
            size={40}
            weight={450}
            color={MUTED}
            style={{ letterSpacing: "-0.01em", lineHeight: 1.3 }}
          />
        </div>
        <div style={{ position: "relative", flex: 1, height: 860 }}>
          {phones.map((p, i) => {
            const t = useRise(8 + i * 8, 30);
            return (
              <Phone
                key={p}
                src={p}
                h={780}
                style={{
                  position: "absolute",
                  left: i * 250,
                  top: 40 + (i % 2) * 40 - interpolate(frame, [0, 150], [0, 30]),
                  opacity: t,
                  translate: `0px ${(1 - t) * 120}px`,
                  rotate: `${(i - 1) * 3}deg`,
                  zIndex: i === 1 ? 2 : 1,
                }}
              />
            );
          })}
        </div>
      </AbsoluteFill>
    </Scene>
  );
};

// ---------- 3. The player, three ways.

export const Player: React.FC = () => {
  const frame = useCurrentFrame();
  const looks = [
    { src: "player_record", label: "A turning record" },
    { src: "player_expressive", label: "Expressive" },
    { src: "player_shape", label: "Material shapes" },
  ];
  return (
    <Scene>
      <AbsoluteFill style={{ alignItems: "center", paddingTop: 70 }}>
        <Headline text="A player that's yours." start={0} size={92} />
        <Headline
          text="Pick the cover, the buttons, and the seek bar."
          start={10}
          size={40}
          weight={450}
          color={MUTED}
          style={{ marginTop: 18, letterSpacing: "-0.01em" }}
        />
        <div style={{ display: "flex", gap: 70, marginTop: 56 }}>
          {looks.map((l, i) => {
            const t = useRise(14 + i * 7, 30);
            return (
              <div key={l.src} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 26, opacity: t, translate: `0px ${(1 - t) * 100}px` }}>
                <Phone src={l.src} h={690} style={{ translate: `0px ${Math.sin((frame + i * 20) / 30) * 6}px` }} />
                <div style={{ fontSize: 32, fontWeight: 600, color: INK }}>{l.label}</div>
              </div>
            );
          })}
        </div>
      </AbsoluteFill>
    </Scene>
  );
};

// ---------- 4. Lyrics, big and moving with the song.

export const Lyrics: React.FC = () => {
  const frame = useCurrentFrame();
  const t = useRise(6, 36);
  return (
    <Scene>
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 130 }}>
        <Phone
          src="lyrics"
          h={900}
          style={{ opacity: t, scale: interpolate(t, [0, 1], [0.9, 1]), rotate: `${interpolate(frame, [0, 150], [-2, 1])}deg` }}
        />
        <div style={{ width: 640 }}>
          <Headline text="Sing along." start={10} size={110} />
          <Headline text="Every word, right on time." start={20} size={52} weight={600} color={AMBER} style={{ marginTop: 20, letterSpacing: "-0.02em" }} />
          <Headline
            text="Timed lyrics follow the song, even word by word. From your files, LRCLIB, or NetEase."
            start={32}
            size={38}
            weight={450}
            color={MUTED}
            style={{ marginTop: 30, letterSpacing: "-0.01em", lineHeight: 1.35 }}
          />
        </div>
      </AbsoluteFill>
    </Scene>
  );
};

// ---------- 5. A quick run through the rest of the app.

export const Tour: React.FC = () => {
  const frame = useCurrentFrame();
  const shots = ["home_night", "library", "albums", "search", "songmenu", "upnext", "appearance_night"];
  // The row slides across slowly, as a carousel.
  const x = interpolate(frame, [0, 180], [220, -1500], { easing: Easing.inOut(Easing.cubic) });
  return (
    <Scene>
      <AbsoluteFill style={{ alignItems: "center", paddingTop: 80 }}>
        <Headline text="Everything, right where you'd look." start={0} size={80} />
      </AbsoluteFill>
      <div style={{ position: "absolute", top: 260, left: 0, display: "flex", gap: 56, translate: `${x}px 0px` }}>
        {shots.map((s, i) => {
          const t = useRise(6 + i * 4, 28);
          return <Phone key={s} src={s} h={720} style={{ opacity: t, translate: `0px ${(1 - t) * 80}px` }} />;
        })}
      </div>
    </Scene>
  );
};

// ---------- 6. The bento: every feature at once.

type Tile = { col: string; row: string; children: React.ReactNode; tint?: string; delay: number };

const Card: React.FC<Tile> = ({ col, row, children, tint = "#1A1816", delay }) => {
  const t = useRise(delay, 26);
  return (
    <div
      style={{
        gridColumn: col,
        gridRow: row,
        background: tint,
        borderRadius: 36,
        overflow: "hidden",
        position: "relative",
        opacity: t,
        scale: interpolate(t, [0, 1], [0.92, 1]),
        boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.06)",
      }}
    >
      {children}
    </div>
  );
};

const Label: React.FC<{ big: string; small?: string; color?: string; size?: number }> = ({ big, small, color = INK, size = 44 }) => (
  <div style={{ position: "absolute", left: 34, bottom: 30, right: 30 }}>
    <div style={{ fontSize: size, fontWeight: 800, color, letterSpacing: "-0.03em", lineHeight: 1.05 }}>{big}</div>
    {small && <div style={{ fontSize: 24, fontWeight: 500, color: MUTED, marginTop: 8, lineHeight: 1.25 }}>{small}</div>}
  </div>
);

/** A crop of a screenshot, for inside a tile. */
const Crop: React.FC<{ src: string; top?: number; w: number; style?: React.CSSProperties }> = ({ src, top = 0, w, style }) => (
  <Img
    src={staticFile(`shots/${src}.png`)}
    style={{ width: w, position: "absolute", top: -top * (w / 1080), borderRadius: 24, ...style }}
  />
);

export const Bento: React.FC = () => {
  const frame = useCurrentFrame();
  // The finale: the grid settles, then the whole of it breathes in a touch.
  const settle = interpolate(frame, [40, 240], [1.03, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: EASE });
  return (
    <Scene>
      <AbsoluteFill style={{ padding: 56, scale: settle }}>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(6, 1fr)",
            gridTemplateRows: "repeat(4, 1fr)",
            gap: 22,
            width: "100%",
            height: "100%",
          }}
        >
          {/* The name: the biggest tile. */}
          <Card col="1 / 3" row="1 / 3" tint="#1F1A12" delay={0}>
            <Img src={staticFile("icon.png")} style={{ position: "absolute", left: 34, top: 34, width: 110, height: 110, borderRadius: 26 }} />
            <div style={{ position: "absolute", left: 34, bottom: 34 }}>
              <div style={{ fontFamily: "Fraunces", fontSize: 92, color: INK, lineHeight: 1 }}>Needle</div>
              <div style={{ fontSize: 44, fontWeight: 700, color: AMBER, marginTop: 8, letterSpacing: "-0.02em" }}>on Android</div>
              <div style={{ fontSize: 24, fontWeight: 500, color: MUTED, marginTop: 18 }}>No account. No ads. No tracking. · needle.nnx.fyi</div>
            </div>
          </Card>

          <Card col="3 / 5" row="1 / 3" tint="#2A1414" delay={4}>
            <Crop src="album2" top={180} w={460} style={{ left: 100, top: 30 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(transparent 45%, #2A1414 80%)" }} />
            <Label big="Cover colours" small="Every page takes its album's colours." />
          </Card>

          <Card col="5 / 7" row="1 / 2" tint="#1C2220" delay={8}>
            <Img src={staticFile("shots/lyrics_crop.png")} style={{ position: "absolute", right: -10, top: -20, width: 270, opacity: 0.85 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(90deg, #1C2220 58%, rgba(28,34,32,0.3) 100%)" }} />
            <Label big="Timed lyrics" small="Word by word, LRCLIB and NetEase" size={40} />
          </Card>
          <Card col="5 / 6" row="2 / 3" tint="#16201A" delay={10}>
            <Label big="Dolby Atmos" small="Played as stereo" size={34} />
          </Card>
          <Card col="6 / 7" row="2 / 3" tint="#1B1A24" delay={12}>
            <Label big="Bit-perfect" small="To USB DACs" size={34} />
          </Card>

          <Card col="1 / 2" row="3 / 5" tint="#1A1816" delay={14}>
            <Crop src="player_record" top={250} w={290} style={{ left: -6, top: 24 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(transparent 40%, #1A1816 75%)" }} />
            <Label big="Your player" small="Record, shape, or square" size={34} />
          </Card>

          <Card col="2 / 4" row="3 / 4" tint="#221C10" delay={16}>
            <Label big="Your computer, from your phone" small="Scan a QR code. Play, skip, search, and turn it up." size={36} />
          </Card>
          <Card col="2 / 3" row="4 / 5" delay={18}>
            <Label big="Navidrome" small="And Subsonic" size={34} />
          </Card>
          <Card col="3 / 4" row="4 / 5" delay={20}>
            <Label big="10-band EQ" small="Crossfeed, balance, mono" size={34} />
          </Card>

          <Card col="4 / 6" row="3 / 5" tint="#101614" delay={22}>
            <Img src={staticFile("shots/widget_crop.png")} style={{ position: "absolute", left: 40, top: 44, width: 520 }} />
            <Label big="Widgets & Android Auto" small="Now playing on your home screen and in your car." size={38} />
          </Card>

          <Card col="6 / 7" row="3 / 4" tint="#1E1520" delay={24}>
            <Label big="Your year" small="Told back" size={34} />
          </Card>
          <Card col="6 / 7" row="4 / 5" delay={26}>
            <Label big="Plugins" small="Themes, effects, sync" size={34} />
          </Card>
        </div>
      </AbsoluteFill>
    </Scene>
  );
};

// ---------- 7. The end card.

export const End: React.FC = () => {
  const t = useRise(6, 34);
  return (
    <Scene>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
        <Img
          src={staticFile("icon.png")}
          style={{ width: 170, height: 170, borderRadius: 40, opacity: t, scale: interpolate(t, [0, 1], [0.85, 1]) }}
        />
        <div style={{ height: 40 }} />
        <Headline text={<span style={{ fontFamily: "Fraunces", fontWeight: 600, letterSpacing: "-0.02em" }}>Needle on Android</span>} start={14} size={120} />
        <Headline text="No account. No ads. No tracking." start={28} size={40} weight={500} color={MUTED} style={{ marginTop: 22, letterSpacing: "-0.01em" }} />
        <Headline text="needle.nnx.fyi" start={40} size={36} weight={600} color={AMBER} style={{ marginTop: 40, letterSpacing: "0em" }} />
      </AbsoluteFill>
    </Scene>
  );
};

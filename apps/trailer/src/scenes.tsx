import React from "react";
import { AbsoluteFill, Easing, Img, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
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
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  // Springs in, tipped back a little, and settles with a soft overshoot; then floats.
  const t = spring({ frame: frame - delay, fps, config: { damping: 13, stiffness: 140, mass: 0.7 } });
  const o = interpolate(frame, [delay, delay + 6], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const float = Math.sin((frame + delay * 7) / 22) * 4;
  return (
    <div style={{ gridColumn: col, gridRow: row, perspective: 1400 }}>
      <div
        style={{
          width: "100%",
          height: "100%",
          background: tint,
          borderRadius: 36,
          overflow: "hidden",
          position: "relative",
          opacity: o,
          transform: `translateY(${(1 - t) * 60 + float}px) scale(${interpolate(t, [0, 1], [0.86, 1])}) rotateX(${(1 - t) * 18}deg)`,
          boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.06)",
        }}
      >
        {children}
        <Sheen at={46 + delay * 2} />
      </div>
    </div>
  );
};

/** A soft band of light that sweeps across a tile once; tile after tile, it reads as one sweep. */
const Sheen: React.FC<{ at: number }> = ({ at }) => {
  const frame = useCurrentFrame();
  const x = interpolate(frame, [at, at + 36], [-40, 140], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.inOut(Easing.cubic) });
  if (frame < at || frame > at + 36) return null;
  return (
    <AbsoluteFill
      style={{
        pointerEvents: "none",
        mixBlendMode: "screen",
        background: `linear-gradient(105deg, transparent ${x - 18}%, rgba(255,240,220,0.10) ${x}%, transparent ${x + 18}%)`,
      }}
    />
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

/** Material icons (rounded set), as SVG paths on a 24-unit grid. */
const ICONS: Record<string, string> = {
  headphones: "M12 1c-4.97 0-9 4.03-9 9v7c0 1.66 1.34 3 3 3h3v-8H5v-2c0-3.87 3.13-7 7-7s7 3.13 7 7v2h-4v8h3c1.66 0 3-1.34 3-3v-7c0-4.97-4.03-9-9-9z",
  usb: "M15 7v4h1v2h-3V5h2l-3-4-3 4h2v8H8v-2.07c.7-.37 1.2-1.08 1.2-1.93 0-1.21-.99-2.2-2.2-2.2-1.21 0-2.2.99-2.2 2.2 0 .85.5 1.56 1.2 1.93V13c0 1.11.89 2 2 2h3v3.05c-.71.37-1.2 1.1-1.2 1.95 0 1.22.99 2.2 2.2 2.2 1.21 0 2.2-.98 2.2-2.2 0-.85-.49-1.58-1.2-1.95V15h3c1.11 0 2-.89 2-2v-2h1V7h-4z",
  computer: "M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z",
  phone: "M17 1.01L7 1c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-1.99-2-1.99zM17 19H7V5h10v14z",
  server: "M20 13H4c-.55 0-1 .45-1 1v6c0 .55.45 1 1 1h16c.55 0 1-.45 1-1v-6c0-.55-.45-1-1-1zM7 19c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zM20 3H4c-.55 0-1 .45-1 1v6c0 .55.45 1 1 1h16c.55 0 1-.45 1-1V4c0-.55-.45-1-1-1zM7 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z",
  tune: "M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z",
  sparkle: "M19 9l1.25-2.75L23 5l-2.75-1.25L19 1l-1.25 2.75L15 5l2.75 1.25L19 9zm-7.5.5L9 4 6.5 9.5 1 12l5.5 2.5L9 20l2.5-5.5L17 12l-5.5-2.5zM19 15l-1.25 2.75L15 19l2.75 1.25L19 23l1.25-2.75L23 19l-2.75-1.25L19 15z",
  extension: "M20.5 11H19V7c0-1.1-.9-2-2-2h-4V3.5C13 2.12 11.88 1 10.5 1S8 2.12 8 3.5V5H4c-1.1 0-1.99.9-1.99 2v3.8H3.5c1.49 0 2.7 1.21 2.7 2.7s-1.21 2.7-2.7 2.7H2V20c0 1.1.9 2 2 2h3.8v-1.5c0-1.49 1.21-2.7 2.7-2.7 1.49 0 2.7 1.21 2.7 2.7V22H17c1.1 0 2-.9 2-2v-4h1.5c1.38 0 2.5-1.12 2.5-2.5S21.88 11 20.5 11z",
  car: "M18.92 6.01C18.72 5.42 18.16 5 17.5 5h-11c-.66 0-1.21.42-1.42 1.01L3 12v8c0 .55.45 1 1 1h1c.55 0 1-.45 1-1v-1h12v1c0 .55.45 1 1 1h1c.55 0 1-.45 1-1v-8l-2.08-5.99zM6.5 16c-.83 0-1.5-.67-1.5-1.5S5.67 13 6.5 13s1.5.67 1.5 1.5S7.33 16 6.5 16zm11 0c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zM5 11l1.5-4.5h11L19 11H5z",
};

/** The app's scalloped "cookie" shape (nine soft bumps), as an SVG path around (50, 50). */
const cookie = (() => {
  const pts: string[] = [];
  for (let i = 0; i <= 180; i++) {
    const a = (i / 180) * Math.PI * 2;
    const r = 44 + 4.5 * Math.cos(a * 9);
    pts.push(`${(50 + r * Math.cos(a)).toFixed(2)},${(50 + r * Math.sin(a)).toFixed(2)}`);
  }
  return `M${pts.join("L")}Z`;
})();

/** An icon in the cookie shape, slowly turning, as the app shows its icons. */
const Badge: React.FC<{ icon: string; color: string; size?: number; style?: React.CSSProperties }> = ({ icon, color, size = 64, style }) => {
  const frame = useCurrentFrame();
  return (
    <div style={{ position: "absolute", left: 26, top: 24, width: size, height: size, ...style }}>
      <svg viewBox="0 0 100 100" width={size} height={size} style={{ position: "absolute", rotate: `${frame * 0.4}deg` }}>
        <path d={cookie} fill={color} />
      </svg>
      <svg viewBox="0 0 24 24" width={size * 0.48} height={size * 0.48} style={{ position: "absolute", left: size * 0.26, top: size * 0.26 }}>
        <path d={ICONS[icon]} fill="#15120E" />
      </svg>
    </div>
  );
};

/** Equalizer bars that move, for the EQ tile. */
const Bars: React.FC<{ color: string }> = ({ color }) => {
  const frame = useCurrentFrame();
  return (
    <div style={{ position: "absolute", right: 28, top: 28, display: "flex", gap: 6, alignItems: "flex-end", height: 56 }}>
      {Array.from({ length: 8 }).map((_, i) => (
        <div
          key={i}
          style={{
            width: 9,
            borderRadius: 5,
            background: color,
            opacity: 0.35 + (i % 3) * 0.2,
            height: 12 + 44 * (0.5 + 0.5 * Math.sin(frame / (6 + i) + i * 1.3)),
          }}
        />
      ))}
    </div>
  );
};

/** A phone and a computer joined by a moving dotted line, for the remote tile. */
const Remote: React.FC<{ color: string }> = ({ color }) => {
  const frame = useCurrentFrame();
  return (
    <div style={{ position: "absolute", left: 30, top: 28, display: "flex", alignItems: "center", gap: 18 }}>
      <Badge icon="phone" color={color} size={70} style={{ position: "relative", left: 0, top: 0 }} />
      <div style={{ display: "flex", gap: 10 }}>
        {Array.from({ length: 7 }).map((_, i) => (
          <div key={i} style={{ width: 9, height: 9, borderRadius: 5, background: color, opacity: 0.25 + 0.75 * Math.max(0, Math.sin(frame / 5 - i * 0.6)) }} />
        ))}
      </div>
      <Badge icon="computer" color={color} size={70} style={{ position: "relative", left: 0, top: 0 }} />
    </div>
  );
};

export const Bento: React.FC = () => {
  const frame = useCurrentFrame();
  // The finale: the grid settles, then the whole of it breathes in a touch.
  // Starts close on the Needle tile, and pulls back to show everything in under a second.
  const pull = interpolate(frame, [0, 24], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.bezier(0.2, 0.9, 0.1, 1) });
  const settle = interpolate(pull, [0, 1], [2.6, 1]) * interpolate(frame, [24, 120], [1, 0.97], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <Scene out={20}>
      <AbsoluteFill style={{ padding: 56, scale: settle, transformOrigin: "18% 26%", filter: pull < 0.98 ? `blur(${(1 - pull) * 6}px)` : undefined }}>
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
          <Card col="1 / 3" row="1 / 3" tint="#1F1A12" delay={6}>
            <Img src={staticFile("icon.png")} style={{ position: "absolute", left: 34, top: 34, width: 110, height: 110, borderRadius: 26 }} />
            <div style={{ position: "absolute", left: 34, bottom: 34 }}>
              <div style={{ fontFamily: "Fraunces", fontSize: 92, color: INK, lineHeight: 1 }}>Needle</div>
              <div style={{ fontSize: 44, fontWeight: 700, color: AMBER, marginTop: 8, letterSpacing: "-0.02em" }}>on Android</div>
              <div style={{ fontSize: 24, fontWeight: 500, color: MUTED, marginTop: 18 }}>No account. No ads. No tracking. · needle.nnx.fyi</div>
            </div>
          </Card>

          <Card col="3 / 5" row="1 / 3" tint="#2A1414" delay={8}>
            <Crop src="album2" top={180} w={460} style={{ left: 100, top: 30 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(transparent 45%, #2A1414 80%)" }} />
            <Label big="Cover colours" small="Every page takes its album's colours." />
          </Card>

          <Card col="5 / 7" row="1 / 2" tint="#1C2220" delay={10}>
            <Img src={staticFile("shots/lyrics_crop.png")} style={{ position: "absolute", right: -10, top: -20, width: 270, opacity: 0.85 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(90deg, #1C2220 58%, rgba(28,34,32,0.3) 100%)" }} />
            <Label big="Timed lyrics" small="Word by word, LRCLIB and NetEase" size={40} />
          </Card>
          <Card col="5 / 6" row="2 / 3" tint="#16201A" delay={11}>
            <Badge icon="headphones" color="#8FD3A8" />
            <Label big="Dolby Atmos" small="Played as stereo" size={34} />
          </Card>
          <Card col="6 / 7" row="2 / 3" tint="#1B1A24" delay={12}>
            <Badge icon="usb" color="#A9A6F0" />
            <Label big="Bit-perfect" small="To USB DACs" size={34} />
          </Card>

          <Card col="1 / 2" row="3 / 5" tint="#1A1816" delay={13}>
            <Crop src="player_record" top={250} w={290} style={{ left: -6, top: 24 }} />
            <div style={{ position: "absolute", inset: 0, background: "linear-gradient(transparent 40%, #1A1816 75%)" }} />
            <Label big="Your player" small="Record, shape, or square" size={34} />
          </Card>

          <Card col="2 / 4" row="3 / 4" tint="#221C10" delay={14}>
            <Remote color={AMBER} />
            <Label big="Your computer, from your phone" small="Scan a QR code. Play, skip, search, and turn it up." size={36} />
          </Card>
          <Card col="2 / 3" row="4 / 5" tint="#151B22" delay={15}>
            <Badge icon="server" color="#8EC1F2" />
            <Label big="Navidrome" small="And Subsonic" size={34} />
          </Card>
          <Card col="3 / 4" row="4 / 5" tint="#221614" delay={16}>
            <Badge icon="tune" color="#F2A68E" />
            <Bars color="#F2A68E" />
            <Label big="10-band EQ" small="Crossfeed, balance, mono" size={34} />
          </Card>

          <Card col="4 / 6" row="3 / 5" tint="#101614" delay={17}>
            <Img src={staticFile("shots/widget_crop.png")} style={{ position: "absolute", left: 40, top: 44, width: 520 }} />
            <Badge icon="car" color="#8FD3C9" size={64} style={{ left: 40, top: 200 }} />
            <Label big="Widgets & Android Auto" small="Now playing on your home screen and in your car." size={38} />
          </Card>

          <Card col="6 / 7" row="3 / 4" tint="#1E1520" delay={18}>
            <Badge icon="sparkle" color="#E7A3E0" />
            <Label big="Your year" small="Told back" size={34} />
          </Card>
          <Card col="6 / 7" row="4 / 5" tint="#1A1E14" delay={19}>
            <Badge icon="extension" color="#C4DB8A" />
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

// ---------- 8. Made by NNX.

export const Maker: React.FC = () => {
  const t = useRise(8, 36);
  return (
    <Scene bg="#000" out={40}>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
        <Img
          src={staticFile("nnx-logo.png")}
          style={{
            width: 420,
            opacity: t,
            scale: interpolate(t, [0, 1], [0.94, 1]),
            filter: `blur(${(1 - t) * 10}px)`,
          }}
        />
      </AbsoluteFill>
    </Scene>
  );
};

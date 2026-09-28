import React from "react";
import { AbsoluteFill, Easing, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { AMBER, EASE, INK, MUTED, Phone } from "./ui";

/*
 * The fast cut: the music is 120 beats a minute, so a beat is 15 frames. Everything moves on
 * the beat; scenes whip in and out sideways with motion blur instead of fading.
 */
export const BEAT = 15;
const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
const SNAP = Easing.bezier(0.2, 0.9, 0.1, 1);

/** How far into the current beat we are, 0 → 1, and which beat it is. */
const useBeat = () => {
  const frame = useCurrentFrame();
  return { n: Math.floor(frame / BEAT), t: (frame % BEAT) / BEAT, frame };
};

/** A little punch at every beat: 1.06 on the beat, back to 1 before the next. */
const punch = (t: number, amount = 0.06) => 1 + amount * Math.pow(1 - t, 3);

/**
 * A scene that whips in from the right and out to the left, blurred as it moves, as a fast
 * camera pan does.
 */
export const Whip: React.FC<{ children: React.ReactNode; bg?: string; enter?: boolean; exit?: boolean }> = ({
  children,
  bg = "#0B0A09",
  enter = true,
  exit = true,
}) => {
  const frame = useCurrentFrame();
  const { durationInFrames, width } = useVideoConfig();
  const inT = enter ? interpolate(frame, [0, 9], [1, 0], { ...clamp, easing: SNAP }) : 0;
  const outT = exit ? interpolate(frame, [durationInFrames - 8, durationInFrames], [0, 1], { ...clamp, easing: Easing.in(Easing.cubic) }) : 0;
  const x = inT * width * 0.35 - outT * width * 0.35;
  const blur = (inT + outT) * 40;
  return (
    <AbsoluteFill style={{ backgroundColor: bg, fontFamily: "Flex", overflow: "hidden" }}>
      <AbsoluteFill style={{ translate: `${x}px 0px`, filter: blur > 0.5 ? `blur(${blur}px)` : undefined }}>{children}</AbsoluteFill>
    </AbsoluteFill>
  );
};

/** Big words that slam in: from large and blurred to their size in a few frames. */
const Slam: React.FC<{ text: React.ReactNode; at: number; size: number; color?: string; font?: string; weight?: number; style?: React.CSSProperties }> = ({
  text,
  at,
  size,
  color = INK,
  font = "Flex",
  weight = 900,
  style,
}) => {
  const frame = useCurrentFrame();
  const t = interpolate(frame, [at, at + 8], [0, 1], { ...clamp, easing: SNAP });
  if (frame < at) return null;
  return (
    <div
      style={{
        fontFamily: font,
        fontSize: size,
        fontWeight: weight,
        color,
        letterSpacing: font === "Flex" ? "-0.045em" : "-0.02em",
        lineHeight: 0.95,
        opacity: t,
        scale: interpolate(t, [0, 1], [1.35, 1]),
        filter: `blur(${(1 - t) * 18}px)`,
        ...style,
      }}
    >
      {text}
    </div>
  );
};

const covers = Array.from({ length: 24 }, (_, i) => `covers/c${String(i).padStart(2, "0")}.jpg`);

// ---------- 1. The build: covers cut faster and faster until the drop.

export const Build: React.FC = () => {
  const frame = useCurrentFrame();
  // A cut every beat, then every half beat, then every quarter: 4 + 4 + 8 + 8 covers.
  const index = frame < 60 ? Math.floor(frame / 15) : frame < 90 ? 4 + Math.floor((frame - 60) / 7.5) : 8 + Math.floor((frame - 90) / 3.75);
  const since = frame < 60 ? frame % 15 : frame < 90 ? (frame - 60) % 7.5 : (frame - 90) % 3.75;
  const cover = covers[index % covers.length];
  const flash = interpolate(frame, [112, 119], [0, 1], clamp);
  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: "hidden", fontFamily: "Flex" }}>
      {/* The cover, huge and blurred, behind itself. */}
      <Img src={staticFile(cover)} style={{ position: "absolute", inset: -200, width: "calc(100% + 400px)", height: "calc(100% + 400px)", objectFit: "cover", filter: "blur(60px) saturate(1.4)", opacity: 0.7 }} />
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
        <Img
          src={staticFile(cover)}
          style={{
            width: 560,
            height: 560,
            borderRadius: 28,
            boxShadow: "0 40px 120px rgba(0,0,0,0.6)",
            scale: 1 + 0.08 * Math.pow(Math.max(0, 1 - since / 6), 2) + frame * 0.0015,
            rotate: `${Math.sin(index * 1.7) * 4}deg`,
          }}
        />
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "flex-end", alignItems: "center", paddingBottom: 90 }}>
        <Slam text="All your music." at={30} size={110} />
      </AbsoluteFill>
      <AbsoluteFill style={{ backgroundColor: "#fff", opacity: flash }} />
    </AbsoluteFill>
  );
};

/** Where the camera looks: from frame `at`, a scale and a point on the screenshot (0–1). */
type Shot = { at: number; s: number; fx: number; fy: number; ox?: number };

/**
 * A camera over a phone: it glides from shot to shot, zooming into the part of the screen
 * that matters, as Apple's product films do. A little motion blur while it travels.
 */
const Camera: React.FC<{ shots: Shot[]; children: React.ReactNode; w: number; h: number; travel?: number }> = ({
  shots,
  children,
  w,
  h,
  travel = 14,
}) => {
  const frame = useCurrentFrame();
  let i = 0;
  while (i + 1 < shots.length && frame >= shots[i + 1].at) i++;
  const cur = shots[i];
  const prev = shots[Math.max(0, i - 1)];
  const k = i === 0 ? 1 : interpolate(frame, [cur.at, cur.at + travel], [0, 1], { ...clamp, easing: SNAP });
  const s = prev.s + (cur.s - prev.s) * k;
  const fx = prev.fx + (cur.fx - prev.fx) * k;
  const fy = prev.fy + (cur.fy - prev.fy) * k;
  // Where on screen the point sits, from the middle: zoomed shots leave room for words.
  const ox = (prev.ox ?? 0) + ((cur.ox ?? 0) - (prev.ox ?? 0)) * k;
  const moving = k > 0 && k < 1 ? Math.sin(k * Math.PI) : 0;
  return (
    <div
      style={{
        transform: `translateX(${ox}px) scale(${s}) translate(${(0.5 - fx) * w}px, ${(0.5 - fy) * h}px)`,
        filter: moving > 0.1 ? `blur(${moving * 3}px)` : undefined,
      }}
    >
      {children}
    </div>
  );
};

/** One clean caption that changes with the shot: rises in, then leaves as the next comes. */
const Caption: React.FC<{ items: { at: number; until: number; big: string; small?: string }[]; style?: React.CSSProperties }> = ({ items, style }) => {
  const frame = useCurrentFrame();
  const item = items.find((c) => frame >= c.at && frame < c.until);
  if (!item) return null;
  const t = interpolate(frame, [item.at, item.at + 10], [0, 1], { ...clamp, easing: SNAP });
  const o = interpolate(frame, [item.until - 6, item.until], [1, 0], clamp);
  return (
    <div style={{ opacity: t * o, translate: `0px ${(1 - t) * 30}px`, filter: `blur(${(1 - t) * 10}px)`, ...style }}>
      <div style={{ fontSize: 88, fontWeight: 850, letterSpacing: "-0.04em", color: INK, lineHeight: 1 }}>{item.big}</div>
      {item.small && <div style={{ fontSize: 34, fontWeight: 500, color: MUTED, marginTop: 16, letterSpacing: "-0.01em" }}>{item.small}</div>}
    </div>
  );
};

// ---------- 2. The drop: the name, the phone, then a tour of its home page up close.

export const Drop: React.FC = () => {
  const frame = useCurrentFrame();
  const white = interpolate(frame, [0, 10], [1, 0], clamp);
  const fly = interpolate(frame, [0, 22], [0, 1], { ...clamp, easing: SNAP });
  const name = interpolate(frame, [30, 44], [1, 0], clamp);
  const H = 900;
  const W = H * (1080 / 2400);
  return (
    <Whip enter={false}>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", opacity: name }}>
        <Slam text="Needle" at={0} size={380} font="Fraunces" weight={600} color="#4A4238" style={{ letterSpacing: "-0.03em" }} />
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
        <Camera
          w={W}
          h={H}
          shots={[
            { at: 0, s: 1, fx: 0.5, fy: 0.5 },
            { at: 45, s: 2.1, fx: 0.4, fy: 0.37, ox: 380 },
            { at: 90, s: 2.6, fx: 0.5, fy: 0.6, ox: 380 },
            { at: 120, s: 2.4, fx: 0.5, fy: 0.9, ox: 380 },
            { at: 150, s: 1.05, fx: 0.5, fy: 0.5 },
          ]}
        >
          <div style={{ perspective: 2200 }}>
            <Phone
              src="home_night"
              h={H}
              style={{ transform: `translateY(${(1 - fly) * 900}px) rotateX(${(1 - fly) * 55}deg) rotateZ(${(1 - fly) * -12}deg)` }}
            />
          </div>
        </Camera>
      </AbsoluteFill>
      <AbsoluteFill
        style={{
          background: "linear-gradient(90deg, rgba(11,10,9,0.95) 0%, rgba(11,10,9,0.8) 30%, transparent 50%)",
          opacity: interpolate(frame, [45, 58, 145, 155], [0, 1, 1, 0], clamp),
        }}
      />
      <Caption
        style={{ position: "absolute", left: 120, top: 440, width: 700 }}
        items={[
          { at: 50, until: 90, big: "Now on Android.", small: "Your library, on your phone." },
          { at: 95, until: 120, big: "Built around you.", small: "Your year, and your computer." },
          { at: 125, until: 150, big: "One tap away.", small: "The player follows you everywhere." },
        ]}
      />
      <AbsoluteFill style={{ backgroundColor: "#fff", opacity: white }} />
    </Whip>
  );
};

// ---------- 3. Colour: every two beats a new album, the whole screen in its colour.

const colourPages = [
  { src: "album", bg: "#E9DFA8", ink: "#4A3F0B" },
  { src: "album2", bg: "#F3B5B0", ink: "#6B1F1A" },
  { src: "artist", bg: "#D9564E", ink: "#FFF1EE" },
  { src: "album", bg: "#E9DFA8", ink: "#4A3F0B" },
];

export const Colour: React.FC = () => {
  const { frame } = useBeat();
  const step = Math.min(3, Math.floor(frame / 30));
  const t = (frame % 30) / 30;
  const page = colourPages[step];
  const swing = interpolate(frame % 30, [0, 10], [1, 0], { ...clamp, easing: SNAP });
  return (
    <Whip bg={page.bg}>
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", padding: "0 140px", gap: 80 }}>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 150, fontWeight: 900, letterSpacing: "-0.05em", lineHeight: 0.92, color: page.ink }}>
            Every
            <br />
            album,
            <br />
            its own
            <br />
            colours.
          </div>
        </div>
        <div style={{ perspective: 2000 }}>
          <Phone
            src={page.src}
            h={900}
            style={{
              transform: `translateX(${swing * 300}px) rotateY(${swing * -35 - 8}deg) scale(${punch(t, 0.03)})`,
              filter: swing > 0.05 ? `blur(${swing * 12}px)` : undefined,
            }}
          />
        </div>
      </AbsoluteFill>
    </Whip>
  );
};

// ---------- 4. The player: four looks, the camera on what each one changes.

const looks = [
  { src: "player_record", big: "A turning record.", fy: 0.35, s: 1.9 },
  { src: "player_shape", big: "Material shapes.", fy: 0.35, s: 1.9 },
  { src: "player_expressive", big: "Expressive buttons.", fy: 0.76, s: 2.3 },
  { src: "player", big: "Or keep it simple.", fy: 0.72, s: 1.7 },
];

export const Player: React.FC = () => {
  const frame = useCurrentFrame();
  const i = Math.min(looks.length - 1, Math.floor(frame / 30));
  const look = looks[i];
  const t = (frame % 30) / 30;
  const H = 900;
  // Each look: arrives close, drifts a little closer while it holds.
  const s = look.s * (1 + 0.06 * t);
  const cut = interpolate(frame % 30, [0, 5], [1, 0], clamp);
  return (
    <Whip bg="#16120C">
      <Img
        src={staticFile(`shots/${look.src}.png`)}
        style={{ position: "absolute", inset: -100, width: "calc(100% + 200px)", height: "calc(100% + 200px)", objectFit: "cover", filter: "blur(80px) saturate(1.3)", opacity: 0.55 }}
      />
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", paddingLeft: 700 }}>
        <div
          style={{
            transform: `scale(${s}) translate(0px, ${(0.5 - look.fy) * H}px)`,
            filter: cut > 0.05 ? `blur(${cut * 14}px)` : undefined,
          }}
        >
          <Phone src={look.src} h={H} />
        </div>
      </AbsoluteFill>
      {/* The words sit on a soft dark wash, clear of the phone. */}
      <AbsoluteFill style={{ background: "linear-gradient(90deg, rgba(10,8,5,0.9) 0%, rgba(10,8,5,0.6) 38%, transparent 60%)" }} />
      <div style={{ position: "absolute", left: 120, top: 380 }}>
        <div style={{ fontSize: 36, fontWeight: 600, color: AMBER, letterSpacing: "-0.01em", marginBottom: 18 }}>Make it yours.</div>
        <div
          key={i}
          style={{
            fontSize: 96,
            fontWeight: 850,
            letterSpacing: "-0.045em",
            lineHeight: 1,
            color: INK,
            width: 640,
            translate: `0px ${interpolate(frame % 30, [0, 9], [36, 0], { ...clamp, easing: SNAP })}px`,
            opacity: interpolate(frame % 30, [0, 6], [0, 1], clamp),
          }}
        >
          {look.big}
        </div>
      </div>
    </Whip>
  );
};

// ---------- 5. Lyrics, huge, moving line by line with the song.

const lines = ["Is it ever 2 late?", "Is it ever 2 late", "베개 끝에 남아 있는", "꿈의 모서리", "커튼 사이 불어오는"];

export const Lyrics: React.FC = () => {
  const frame = useCurrentFrame();
  // A line every two beats; the list glides up as each takes its turn.
  const pos = interpolate(frame, [0, 90], [0, 3], { easing: Easing.inOut(Easing.cubic) });
  return (
    <Whip bg="#1E2A22">
      <Img src={staticFile("shots/lyrics.png")} style={{ position: "absolute", inset: -100, width: "calc(100% + 200px)", height: "calc(100% + 200px)", objectFit: "cover", filter: "blur(90px) saturate(1.4)", opacity: 0.6 }} />
      <AbsoluteFill style={{ justifyContent: "center", paddingLeft: 140 }}>
        {lines.map((l, i) => {
          const d = i - pos;
          return (
            <div
              key={l}
              style={{
                position: "absolute",
                top: 470 + d * 170,
                fontSize: 128,
                fontWeight: 900,
                letterSpacing: "-0.04em",
                color: INK,
                opacity: interpolate(Math.abs(d), [0, 0.6, 2.5], [1, 0.35, 0], clamp),
                scale: interpolate(Math.abs(d), [0, 1], [1, 0.9], clamp),
                transformOrigin: "left center",
              }}
            >
              {l}
            </div>
          );
        })}
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "flex-end", alignItems: "flex-end", padding: 80 }}>
        <div style={{ fontSize: 44, fontWeight: 700, color: AMBER, letterSpacing: "-0.02em" }}>Lyrics that keep time.</div>
      </AbsoluteFill>
    </Whip>
  );
};

// ---------- 6. Coverflow: the rest of the app, one screen per beat.

const flow = [
  { src: "home_night", name: "Home" },
  { src: "library", name: "Library" },
  { src: "albums", name: "Albums" },
  { src: "search", name: "Search" },
  { src: "artist", name: "Artists" },
  { src: "songmenu", name: "Menus" },
  { src: "upnext", name: "Up next" },
  { src: "appearance_night", name: "Appearance" },
];

export const Carousel: React.FC = () => {
  const frame = useCurrentFrame();
  // One step a beat: a quick glide on the beat, then it holds.
  const step = Math.min(flow.length - 1, Math.floor(frame / BEAT));
  const glide = interpolate(frame % BEAT, [0, 9], [0, 1], { ...clamp, easing: SNAP });
  const pos = frame < BEAT ? 0 : Math.min(flow.length - 1, step - 1 + glide);
  const current = flow[Math.round(pos)];
  const H = 660;
  return (
    <Whip bg="#070605">
      {/* The screen in front tints the room. */}
      <Img
        key={current.src}
        src={staticFile(`shots/${current.src}.png`)}
        style={{ position: "absolute", left: "25%", top: "5%", width: "50%", height: "80%", objectFit: "cover", filter: "blur(140px) saturate(1.4)", opacity: 0.35 }}
      />
      <AbsoluteFill style={{ alignItems: "center", paddingTop: 64 }}>
        <div style={{ fontSize: 30, fontWeight: 600, color: AMBER, letterSpacing: "0.01em" }}>Everything in its place</div>
      </AbsoluteFill>
      <AbsoluteFill style={{ perspective: 1800, alignItems: "center", justifyContent: "center", top: 40 }}>
        {flow.map((f, i) => {
          const d = i - pos;
          const a = Math.min(1, Math.abs(d));
          const side = Math.sign(d);
          const x = d * 130 + side * a * 220;
          return (
            <div
              key={f.src}
              style={{
                position: "absolute",
                zIndex: 100 - Math.round(Math.abs(d) * 10),
                transform: `translateX(${x}px) translateZ(${-a * 260}px) rotateY(${-side * a * 50}deg)`,
                opacity: interpolate(Math.abs(d), [3, 4], [1, 0], clamp),
                filter: `brightness(${1 - a * 0.45})`,
                WebkitBoxReflect: "below 14px linear-gradient(transparent 72%, rgba(255,255,255,0.22))",
              }}
            >
              <Phone src={f.src} h={H} />
            </div>
          );
        })}
      </AbsoluteFill>
      <AbsoluteFill style={{ alignItems: "center", paddingTop: 108 }}>
        <div
          key={current.name}
          style={{
            fontSize: 64,
            fontWeight: 850,
            letterSpacing: "-0.04em",
            color: INK,
            opacity: interpolate(frame % BEAT, [0, 5], [frame < BEAT ? 1 : 0.2, 1], clamp),
            translate: `0px ${interpolate(frame % BEAT, [0, 8], [frame < BEAT ? 0 : 14, 0], { ...clamp, easing: SNAP })}px`,
          }}
        >
          {current.name}
        </div>
      </AbsoluteFill>
    </Whip>
  );
};

// ---------- 8. Made by NNX.

export const Maker: React.FC = () => {
  const frame = useCurrentFrame();
  const t = interpolate(frame, [4, 22], [0, 1], { ...clamp, easing: EASE });
  const out = interpolate(frame, [70, 110], [1, 0], clamp);
  return (
    <AbsoluteFill style={{ backgroundColor: "#000", justifyContent: "center", alignItems: "center" }}>
      <Img
        src={staticFile("nnx-logo.png")}
        style={{ width: 420, opacity: t * out, scale: interpolate(t, [0, 1], [1.25, 1]), filter: `blur(${(1 - t) * 14}px)` }}
      />
    </AbsoluteFill>
  );
};

export { MUTED };

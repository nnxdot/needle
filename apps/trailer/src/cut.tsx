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

// ---------- 2. The drop: the name slams in and the phone flies up.

export const Drop: React.FC = () => {
  const frame = useCurrentFrame();
  const white = interpolate(frame, [0, 10], [1, 0], clamp);
  const fly = interpolate(frame, [0, 22], [0, 1], { ...clamp, easing: SNAP });
  const push = interpolate(frame, [0, 180], [1, 1.1], clamp);
  return (
    <Whip enter={false}>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", scale: push }}>
        <Slam
          text="Needle"
          at={0}
          size={380}
          font="Fraunces"
          weight={600}
          color="#4A4238"
          style={{ position: "absolute", top: 250, letterSpacing: "-0.03em" }}
        />
        <div style={{ perspective: 2200 }}>
          <Phone
            src="home_night"
            h={900}
            style={{
              transform: `translateY(${(1 - fly) * 900}px) rotateX(${(1 - fly) * 55}deg) rotateZ(${(1 - fly) * -12}deg) rotateY(${Math.sin(frame / 40) * 6}deg)`,
            }}
          />
        </div>
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "center", paddingLeft: 120 }}>
        <Slam text={<>Now on<br />Android.</>} at={45} size={120} />
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "flex-end", paddingRight: 140 }}>
        <Slam text={<>Made for<br />your music.</>} at={75} size={120} color={AMBER} style={{ textAlign: "right" }} />
      </AbsoluteFill>
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

// ---------- 4. The player: its look changes on every beat.

const looks = [
  { src: "player_record", word: "Record." },
  { src: "player_shape", word: "Shape." },
  { src: "player_expressive", word: "Square." },
  { src: "player", word: "Minimal." },
];

export const Player: React.FC = () => {
  const { n, t, frame } = useBeat();
  const look = looks[n % looks.length];
  return (
    <Whip bg="#16120C">
      <Img
        src={staticFile(`shots/${look.src}.png`)}
        style={{ position: "absolute", inset: -100, width: "calc(100% + 200px)", height: "calc(100% + 200px)", objectFit: "cover", filter: "blur(80px) saturate(1.3)", opacity: 0.55 }}
      />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 120 }}>
        <div style={{ perspective: 2000 }}>
          <Phone src={look.src} h={940} style={{ transform: `rotateY(${10 - frame * 0.1}deg) scale(${punch(t, 0.04)})` }} />
        </div>
        <div style={{ width: 700 }}>
          <div style={{ fontSize: 120, fontWeight: 900, letterSpacing: "-0.05em", lineHeight: 0.95, color: INK }}>
            Make it
            <br />
            yours.
          </div>
          <div
            key={n}
            style={{
              fontSize: 120,
              fontWeight: 900,
              letterSpacing: "-0.05em",
              color: AMBER,
              marginTop: 10,
              translate: `0px ${interpolate(t, [0, 0.35], [40, 0], { ...clamp, easing: SNAP })}px`,
              opacity: interpolate(t, [0, 0.25], [0, 1], clamp),
            }}
          >
            {look.word}
          </div>
        </div>
      </AbsoluteFill>
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

// ---------- 6. A 3D carousel of everything else, spinning.

const ring = ["home_night", "library", "albums", "search", "songmenu", "upnext", "appearance_night", "settings"];

export const Carousel: React.FC = () => {
  const frame = useCurrentFrame();
  const spin = interpolate(frame, [0, 120], [0, 200], { easing: Easing.inOut(Easing.quad) });
  const radius = 1350;
  return (
    <Whip bg="#0B0A09">
      <AbsoluteFill style={{ alignItems: "center", paddingTop: 70 }}>
        <div style={{ fontSize: 96, fontWeight: 900, letterSpacing: "-0.045em", color: INK }}>All of it. Beautifully.</div>
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", perspective: 2600, top: 140 }}>
        <div style={{ position: "relative", width: 1, height: 1, transformStyle: "preserve-3d", transform: `translateZ(${-radius}px) rotateY(${-spin}deg)` }}>
          {ring.map((s, i) => {
            const a = (i / ring.length) * 360;
            return (
              <div key={s} style={{ position: "absolute", left: -180, top: -390, transform: `rotateY(${a}deg) translateZ(${radius}px)`, backfaceVisibility: "hidden" }}>
                <Phone src={s} h={740} />
              </div>
            );
          })}
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

import React from "react";
import { AbsoluteFill, Easing, Img, interpolate, interpolateColors, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { AMBER, EASE, INK, MUTED, Phone } from "./ui";

/*
 * The fast cut: the music is 120 beats a minute, so a beat is 15 frames. Everything moves on
 * the beat; scenes whip in and out sideways with motion blur instead of fading.
 */
export const BEAT = 15;
const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
const SNAP = Easing.bezier(0.2, 0.9, 0.1, 1);

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
  // It leans into the move and pulses a touch on every beat, so nothing ever stands still.
  const scale = (1 - (inT + outT) * 0.06) * pulse(frame);
  return (
    <AbsoluteFill style={{ backgroundColor: bg, fontFamily: "Flex", overflow: "hidden" }}>
      <AbsoluteFill style={{ translate: `${x}px 0px`, scale, filter: blur > 0.5 ? `blur(${blur}px)` : undefined }}>{children}</AbsoluteFill>
    </AbsoluteFill>
  );
};

/** A small push on each beat that dies away before the next. */
export const pulse = (frame: number, amount = 0.01) => 1 + amount * Math.pow(1 - (frame % BEAT) / BEAT, 3);

/**
 * Words that arrive one after another: each rises, sharpens, and settles, a few frames after
 * the one before. "\n" starts a new line. With `letters`, it goes letter by letter.
 */
export const Words: React.FC<{
  text: string;
  at: number;
  stagger?: number;
  letters?: boolean;
  slam?: boolean;
  until?: number;
  style?: React.CSSProperties;
}> = ({ text, at, stagger = 2.5, letters = false, slam = false, until, style }) => {
  const frame = useCurrentFrame();
  let n = 0;
  const leave = until === undefined ? 0 : interpolate(frame, [until - 7, until], [0, 1], { ...clamp, easing: Easing.in(Easing.cubic) });
  return (
    <div style={style}>
      {text.split("\n").map((line, li) => (
        <div key={li}>
          {(letters ? [...line] : line.split(" ")).map((w, wi) => {
            const start = at + n++ * stagger;
            const t = interpolate(frame, [start, start + (slam ? 8 : 13)], [0, 1], { ...clamp, easing: SNAP });
            return (
              <span
                key={wi}
                style={{
                  display: "inline-block",
                  whiteSpace: "pre",
                  opacity: t * (1 - leave),
                  translate: slam ? undefined : `0px ${(1 - t) * 0.45 - leave * 0.3}em`,
                  scale: slam ? interpolate(t, [0, 1], [1.5, 1]) : undefined,
                  filter: t < 1 || leave > 0 ? `blur(${(1 - t) * (slam ? 18 : 10) + leave * 10}px)` : undefined,
                  marginRight: letters ? 0 : "0.24em",
                }}
              >
                {w}
              </span>
            );
          })}
        </div>
      ))}
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
  const previous = index > 0 ? covers[(index - 1) % covers.length] : null;
  const flash = interpolate(frame, [112, 119], [0, 1], clamp);
  // The new cover lands (from a little small and turned); the old one flies past the camera.
  const land = interpolate(since, [0, 5], [0, 1], { ...clamp, easing: SNAP });
  const tilt = (i: number) => Math.sin(i * 1.7) * 4;
  // The whole stack drifts slowly and turns in 3D, faster as the build speeds up.
  const drift = frame * 0.0015;
  const sway = Math.sin(frame / 18) * 6;
  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: "hidden", fontFamily: "Flex" }}>
      {/* The cover, huge and blurred, behind itself; the old one fades under the new. */}
      {previous && <Img src={staticFile(previous)} style={{ position: "absolute", inset: -200, width: "calc(100% + 400px)", height: "calc(100% + 400px)", objectFit: "cover", filter: "blur(60px) saturate(1.4)", opacity: 0.7 }} />}
      <Img src={staticFile(cover)} style={{ position: "absolute", inset: -200, width: "calc(100% + 400px)", height: "calc(100% + 400px)", objectFit: "cover", filter: "blur(60px) saturate(1.4)", opacity: 0.7 * land, scale: 1.1 - 0.1 * land }} />
      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", perspective: 1600 }}>
        <div style={{ position: "relative", width: 560, height: 560, transform: `rotateY(${sway}deg) rotateX(${-sway * 0.4}deg)` }}>
          <Img
            src={staticFile(cover)}
            style={{
              position: "absolute",
              width: 560,
              height: 560,
              borderRadius: 28,
              boxShadow: "0 40px 120px rgba(0,0,0,0.6)",
              scale: interpolate(land, [0, 1], [0.86, 1]) + 0.08 * Math.pow(Math.max(0, 1 - since / 6), 2) + drift,
              rotate: `${interpolate(land, [0, 1], [tilt(index) * 3, tilt(index)])}deg`,
              opacity: interpolate(land, [0, 0.4], [0.3, 1], clamp),
            }}
          />
          {previous && land < 1 && (
            <Img
              src={staticFile(previous)}
              style={{
                position: "absolute",
                width: 560,
                height: 560,
                borderRadius: 28,
                scale: 1 + drift + land * 0.9,
                rotate: `${tilt(index - 1)}deg`,
                opacity: 1 - land,
                filter: `blur(${land * 16}px)`,
              }}
            />
          )}
        </div>
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "flex-end", alignItems: "center", paddingBottom: 90 }}>
        <Words text="All your music." at={30} stagger={4} slam style={{ fontSize: 110, fontWeight: 900, color: INK, letterSpacing: "-0.045em", lineHeight: 0.95 }} />
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
  return (
    <div key={item.at} style={style}>
      <Words text={item.big} at={item.at} until={item.until} style={{ fontSize: 88, fontWeight: 850, letterSpacing: "-0.04em", color: INK, lineHeight: 1 }} />
      {item.small && (
        <Words
          text={item.small}
          at={item.at + 6}
          stagger={1.5}
          until={item.until}
          style={{ fontSize: 34, fontWeight: 500, color: MUTED, marginTop: 16, letterSpacing: "-0.01em" }}
        />
      )}
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
        <Words
          text="Needle"
          at={0}
          stagger={1.5}
          letters
          slam
          style={{ fontFamily: "Fraunces", fontSize: 380, fontWeight: 600, color: "#4A4238", letterSpacing: "-0.03em", lineHeight: 0.95, scale: 1 + frame * 0.002 }}
        />
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
              style={{
                transform: `translateY(${(1 - fly) * 900}px) rotateX(${(1 - fly) * 55 + Math.sin(frame / 22) * 3}deg) rotateY(${Math.sin(frame / 30) * 7}deg) rotateZ(${(1 - fly) * -12}deg)`,
              }}
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

// ---------- 3. Colour: the cover's colour pours out of it and fills the screen, then the page.

const colourPages = [
  { src: "album", bg: "#E9DC9A", ink: "#3E3408" },
  { src: "album2", bg: "#F2AFAA", ink: "#5E1814" },
  { src: "album3", bg: "#8E9BB3", ink: "#141B2B" },
];

export const Colour: React.FC = () => {
  const frame = useCurrentFrame();
  const { width, height } = useVideoConfig();
  const STEP = 40;
  const i = Math.min(colourPages.length - 1, Math.floor(frame / STEP));
  const f = frame - i * STEP;
  const page = colourPages[i];
  const before = i > 0 ? colourPages[i - 1].bg : "#0B0A09";
  const coverX = width * 0.66;
  const coverY = height * 0.5;
  // 1. The cover pops in, in the middle of where the phone will be.
  const pop = interpolate(f, [0, 7], [0, 1], { ...clamp, easing: SNAP });
  // 2. Its colour floods out of it in a circle.
  const flood = interpolate(f, [4, 16], [0, 1], { ...clamp, easing: Easing.bezier(0.5, 0, 0.1, 1) });
  // 3. The cover shrinks into the phone as its page slides in around it.
  const settle = interpolate(f, [14, 26], [0, 1], { ...clamp, easing: SNAP });
  const radius = Math.hypot(width, height) * flood;
  const H = 880;
  // The words change to the new album's ink as its colour reaches them.
  const inkBefore = i > 0 ? colourPages[i - 1].ink : INK;
  const ink = interpolateColors(flood, [0.45, 0.75], [inkBefore, page.ink]);
  return (
    <Whip bg={before}>
      <AbsoluteFill style={{ backgroundColor: page.bg, clipPath: `circle(${radius}px at ${coverX}px ${coverY}px)` }} />
      {/* The headline, in the album's own ink. */}
      <AbsoluteFill style={{ justifyContent: "center", paddingLeft: 140 }}>
        <Words text="Cover colours" at={2} style={{ fontSize: 34, fontWeight: 650, color: ink, opacity: 0.7, marginBottom: 20, letterSpacing: "-0.01em" }} />
        <Words
          text={"Every album\nwears its\nown colours."}
          at={5}
          stagger={2}
          style={{ fontSize: 104, fontWeight: 850, letterSpacing: "-0.045em", lineHeight: 0.98, color: ink, width: 760 }}
        />
      </AbsoluteFill>
      {/* The phone with the album's page arrives as the colour settles, turning to face you. */}
      <div
        style={{
          position: "absolute",
          left: coverX - (H * (1080 / 2400)) / 2 - H * 0.014,
          top: coverY - H / 2,
          opacity: settle,
          perspective: 2000,
        }}
      >
        <Phone
          src={page.src}
          h={H}
          style={{
            transform: `translateY(${(1 - settle) * 60}px) rotateY(${(1 - settle) * -28 + Math.sin(frame / 25) * 3}deg) rotateX(${(1 - settle) * 10}deg)`,
          }}
        />
      </div>
      {/* The cover itself, large, then gone into the page. */}
      <Img
        src={staticFile(`shots/${page.src}_cover.png`)}
        style={{
          position: "absolute",
          width: 520,
          height: 520,
          left: coverX - 260,
          top: coverY - 260,
          borderRadius: 26,
          boxShadow: "0 40px 100px rgba(0,0,0,0.35)",
          opacity: pop * interpolate(settle, [0.7, 1], [1, 0], clamp),
          // Lands exactly on the cover inside the phone's page (257 px wide, 211 px above the middle).
          scale: interpolate(pop, [0, 1], [0.6, 1]) * interpolate(settle, [0, 1], [1, 0.49]),
          translate: `0px ${settle * -211}px`,
          rotate: `${(1 - pop) * -8}deg`,
        }}
      />
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
  const f = frame - i * 30;
  // Each new look slides up into place as the last one leaves upward.
  const k = i === 0 ? 1 : interpolate(f, [0, 10], [0, 1], { ...clamp, easing: SNAP });
  const H = 900;
  const layer = (j: number, y: number, fade: number, since: number) => {
    const look = looks[j];
    // Arrives close, drifts a little closer while it holds, and sways gently.
    const s = look.s * (1 + 0.06 * (since / 30));
    const speed = Math.sin(Math.min(1, k) * Math.PI);
    return (
      <AbsoluteFill key={look.src} style={{ justifyContent: "center", alignItems: "center", paddingLeft: 700, perspective: 2400 }}>
        <div
          style={{
            opacity: fade,
            transform: `translateY(${y}px) scale(${s}) translate(0px, ${(0.5 - look.fy) * H}px) rotateY(${Math.sin((frame + j * 20) / 24) * 4}deg)`,
            filter: speed > 0.1 && i > 0 ? `blur(${speed * 8}px)` : undefined,
          }}
        >
          <Phone src={look.src} h={H} />
        </div>
      </AbsoluteFill>
    );
  };
  const glow = (j: number, o: number) => (
    <Img
      key={`glow-${j}`}
      src={staticFile(`shots/${looks[j].src}.png`)}
      style={{ position: "absolute", inset: -100, width: "calc(100% + 200px)", height: "calc(100% + 200px)", objectFit: "cover", filter: "blur(80px) saturate(1.3)", opacity: 0.55 * o }}
    />
  );
  return (
    <Whip bg="#16120C">
      {i > 0 && k < 1 && glow(i - 1, 1)}
      {glow(i, k)}
      {i > 0 && k < 1 && layer(i - 1, -k * 1300, 1 - k * 0.6, f + 30)}
      {layer(i, (1 - k) * 1300, 1, f)}
      {/* The words sit on a soft dark wash, clear of the phone. */}
      <AbsoluteFill style={{ background: "linear-gradient(90deg, rgba(10,8,5,0.9) 0%, rgba(10,8,5,0.6) 38%, transparent 60%)" }} />
      <div style={{ position: "absolute", left: 120, top: 380 }}>
        <Words text="Make it yours." at={0} stagger={2} style={{ fontSize: 36, fontWeight: 600, color: AMBER, letterSpacing: "-0.01em", marginBottom: 18 }} />
        <Words
          key={i}
          text={looks[i].big}
          at={i * 30 + 1}
          stagger={2.5}
          until={i < looks.length - 1 ? i * 30 + 30 : undefined}
          style={{ fontSize: 96, fontWeight: 850, letterSpacing: "-0.045em", lineHeight: 1, color: INK, width: 640 }}
        />
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
      <Img
        src={staticFile("shots/lyrics.png")}
        style={{
          position: "absolute",
          inset: -200,
          width: "calc(100% + 400px)",
          height: "calc(100% + 400px)",
          objectFit: "cover",
          filter: "blur(90px) saturate(1.4)",
          opacity: 0.6,
          rotate: `${frame * 0.25}deg`,
          scale: 1.2 + Math.sin(frame / 20) * 0.05,
        }}
      />
      <AbsoluteFill style={{ justifyContent: "center", paddingLeft: 140 }}>
        {lines.map((l, i) => {
          const d = i - pos;
          // As a line takes its turn, its words light up one after another, as sung.
          const words = l.split(" ");
          const sung = (pos - i + 0.7) * (words.length + 1) * 1.1;
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
                filter: Math.abs(d) > 0.3 ? `blur(${Math.min(6, (Math.abs(d) - 0.3) * 4)}px)` : undefined,
              }}
            >
              {words.map((w, wi) => {
                const lit = interpolate(sung - wi, [0, 1], [0.45, 1], clamp);
                return (
                  <span key={wi} style={{ display: "inline-block", marginRight: "0.24em", opacity: lit, translate: `0px ${(1 - lit) * 6}px` }}>
                    {w}
                  </span>
                );
              })}
            </div>
          );
        })}
      </AbsoluteFill>
      <AbsoluteFill style={{ justifyContent: "flex-end", alignItems: "flex-end", padding: 80 }}>
        <Words text="Lyrics that keep time." at={8} style={{ fontSize: 44, fontWeight: 700, color: AMBER, letterSpacing: "-0.02em" }} />
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
  // Most of the step lands on the beat; the rest creeps on, so the row never quite stops.
  const glide = interpolate(frame % BEAT, [0, 9], [0, 0.85], { ...clamp, easing: SNAP }) + 0.15 * ((frame % BEAT) / BEAT);
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
                transform: `translateX(${x}px) translateY(${Math.sin(frame / 14 + i * 1.3) * 8}px) translateZ(${-a * 260}px) rotateY(${-side * a * 50}deg)`,
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
            filter: frame >= BEAT && frame % BEAT < 6 ? `blur(${(1 - (frame % BEAT) / 6) * 8}px)` : undefined,
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

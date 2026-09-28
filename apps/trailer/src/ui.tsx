import React from "react";
import { loadFont } from "@remotion/fonts";
import {
  AbsoluteFill,
  Easing,
  Img,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";

// The app's own fonts: Roboto Flex for words, Fraunces for the name.
loadFont({ family: "Flex", url: staticFile("roboto_flex.ttf"), weight: "100 1000" });
loadFont({ family: "Fraunces", url: staticFile("Fraunces72ptSoft-SemiBold.ttf") });

export const INK = "#F5F3EF";
export const MUTED = "#9C968F";
export const AMBER = "#E2B46C";
export const PAGE = "#0B0A09";

/** Apple's ease: quick out, long settle. */
export const EASE = Easing.bezier(0.16, 1, 0.3, 1);

/** 0 → 1 over [start, start + length] frames, eased. */
export const useRise = (start: number, length = 24) => {
  const frame = useCurrentFrame();
  return interpolate(frame, [start, start + length], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
    easing: EASE,
  });
};

/** Fades a whole scene in at its start and out at its end. */
export const Scene: React.FC<{ children: React.ReactNode; bg?: string; out?: number }> = ({ children, bg = PAGE, out = 10 }) => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();
  const opacity = interpolate(frame, [0, 10, durationInFrames - out, durationInFrames], [0, 1, 1, 0], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });
  return <AbsoluteFill style={{ backgroundColor: bg, opacity, fontFamily: "Flex" }}>{children}</AbsoluteFill>;
};

/** A headline that rises into place, as on Apple's pages. */
export const Headline: React.FC<{
  text: React.ReactNode;
  start?: number;
  size?: number;
  color?: string;
  weight?: number;
  width?: number;
  style?: React.CSSProperties;
}> = ({ text, start = 0, size = 96, color = INK, weight = 800, width = 100, style }) => {
  const t = useRise(start, 28);
  return (
    <div
      style={{
        fontSize: size,
        fontWeight: weight,
        color,
        letterSpacing: "-0.035em",
        lineHeight: 1.02,
        fontVariationSettings: `"wdth" ${width}`,
        opacity: t,
        translate: `0px ${(1 - t) * 40}px`,
        filter: `blur(${(1 - t) * 8}px)`,
        ...style,
      }}
    >
      {text}
    </div>
  );
};

/**
 * A phone: the emulator's screenshot in a slim black frame with rounded corners, as a
 * product shot. `h` is its height in pixels.
 */
export const Phone: React.FC<{ src: string; h?: number; style?: React.CSSProperties }> = ({ src, h = 900, style }) => {
  const w = h * (1080 / 2400);
  const bezel = h * 0.014;
  return (
    <div
      style={{
        width: w + bezel * 2,
        height: h + bezel * 2,
        borderRadius: h * 0.075,
        padding: bezel,
        background: "linear-gradient(145deg, #3a3835, #121110 40%, #2a2826)",
        boxShadow: `0 ${h * 0.04}px ${h * 0.12}px rgba(0,0,0,0.55), inset 0 0 0 1.5px rgba(255,255,255,0.08)`,
        ...style,
      }}
    >
      <Img
        src={staticFile(`shots/${src}.png`)}
        style={{ width: w, height: h, borderRadius: h * 0.062, display: "block", objectFit: "cover" }}
      />
    </div>
  );
};

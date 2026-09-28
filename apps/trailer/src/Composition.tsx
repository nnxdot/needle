import { Audio } from "@remotion/media";
import { Composition, interpolate, Series, staticFile } from "remotion";
import { Bento, Colour, Intro, Lyrics, Maker, Player, Tour } from "./scenes";

// The music: Justice, Tame Impala, "Neverender" (Rampa Remix), 120 beats a minute, so a bar
// is 2 s (60 frames). It starts 4 s before its drop (1:52.017), and the drop lands as the first
// scene of the app appears; every scene is a whole number of bars.
const FPS = 30;
const DROP = 112.017;
const LEAD = 4;
const LENGTH = 1140;

export const Trailer: React.FC = () => (
  <>
    <Audio
      src={staticFile("music.m4a")}
      trimBefore={Math.round((DROP - LEAD) * FPS)}
      volume={(f) =>
        interpolate(f, [0, 20, LENGTH - 90, LENGTH], [0, 1, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
      }
    />
    <Series>
      <Series.Sequence durationInFrames={120} name="Intro">
        <Intro />
      </Series.Sequence>
      <Series.Sequence durationInFrames={180} name="Colour">
        <Colour />
      </Series.Sequence>
      <Series.Sequence durationInFrames={180} name="Player">
        <Player />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Lyrics">
        <Lyrics />
      </Series.Sequence>
      <Series.Sequence durationInFrames={180} name="Tour">
        <Tour />
      </Series.Sequence>
      <Series.Sequence durationInFrames={240} name="Bento">
        <Bento />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Maker">
        <Maker />
      </Series.Sequence>
    </Series>
  </>
);

export const MyComposition = () => (
  <Composition id="NeedleAndroid" component={Trailer} durationInFrames={LENGTH} fps={FPS} width={1920} height={1080} />
);

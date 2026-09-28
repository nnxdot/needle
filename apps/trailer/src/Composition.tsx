import { Audio } from "@remotion/media";
import { Composition, interpolate, Series, staticFile } from "remotion";
import { Build, Carousel, Colour, Drop, Lyrics, Maker, Player } from "./cut";
import { Bento } from "./scenes";

// The music: Justice, Tame Impala, "Neverender" (Rampa Remix), 120 beats a minute: a beat is
// 15 frames. It starts 4 s before its drop (1:52.017); the covers cut faster and faster over
// the build, and the drop lands on the name. Every scene is a whole number of bars.
const FPS = 30;
const DROP = 112.017;
const LEAD = 4;
const LENGTH = 990;

export const Trailer: React.FC = () => (
  <>
    <Audio
      src={staticFile("music.m4a")}
      trimBefore={Math.round((DROP - LEAD) * FPS)}
      volume={(f) =>
        interpolate(f, [0, 15, LENGTH - 90, LENGTH], [0, 1, 1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
      }
    />
    <Series>
      <Series.Sequence durationInFrames={120} name="Build">
        <Build />
      </Series.Sequence>
      <Series.Sequence durationInFrames={180} name="Drop">
        <Drop />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Colour">
        <Colour />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Player">
        <Player />
      </Series.Sequence>
      <Series.Sequence durationInFrames={90} name="Lyrics">
        <Lyrics />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Carousel">
        <Carousel />
      </Series.Sequence>
      <Series.Sequence durationInFrames={120} name="Bento">
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

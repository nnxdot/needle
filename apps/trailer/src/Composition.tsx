import { Composition, Series } from "remotion";
import { Bento, Colour, Intro, Lyrics, Player, Tour } from "./scenes";

export const Trailer: React.FC = () => (
  <Series>
    <Series.Sequence durationInFrames={105} name="Intro">
      <Intro />
    </Series.Sequence>
    <Series.Sequence durationInFrames={150} name="Colour">
      <Colour />
    </Series.Sequence>
    <Series.Sequence durationInFrames={150} name="Player">
      <Player />
    </Series.Sequence>
    <Series.Sequence durationInFrames={135} name="Lyrics">
      <Lyrics />
    </Series.Sequence>
    <Series.Sequence durationInFrames={180} name="Tour">
      <Tour />
    </Series.Sequence>
    <Series.Sequence durationInFrames={240} name="Bento">
      <Bento />
    </Series.Sequence>
  </Series>
);

export const MyComposition = () => (
  <Composition
    id="NeedleAndroid"
    component={Trailer}
    durationInFrames={960}
    fps={30}
    width={1920}
    height={1080}
  />
);

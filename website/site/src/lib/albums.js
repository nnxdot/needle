// Made-up albums for the drawings of the app. The covers are free photos from Unsplash
// (through picsum.photos), used under the Unsplash license.
export const albums = [
  { id: "harbor-lights", title: "Harbor Lights", artist: "Mara Quinn", year: 2021, glow: "#f2a25e" },
  { id: "tidewater", title: "Tidewater", artist: "Mara Quinn", year: 2024, glow: "#4a92f2" },
  { id: "afterglow-arcade", title: "Afterglow Arcade", artist: "Pixel Choir", year: 2023, glow: "#ec6a86" },
  { id: "single-needle", title: "Single Needle", artist: "The Pressings", year: 1998, glow: "#c9c9cf" },
  { id: "bloom-season", title: "Bloom Season", artist: "Hana Mori", year: 2025, glow: "#f28d6c" },
  { id: "berry-season", title: "Berry Season", artist: "Juniper Bay", year: 2022, glow: "#e8505e" },
  { id: "understory", title: "Understory", artist: "Fern & Field", year: 2019, glow: "#c9b064" },
  { id: "grey-weather", title: "Grey Weather", artist: "Low Ceiling", year: 2017, glow: "#7fa4c0" },
  { id: "red-light-only", title: "Red Light Only", artist: "Darkroom", year: 2020, glow: "#e8a052" },
  { id: "night-ferry", title: "Night Ferry", artist: "Oda Lind", year: 2024, glow: "#8f78e8" },
  { id: "low-desert", title: "Low Desert", artist: "Sol Arden", year: 2018, glow: "#e08848" },
  { id: "middle-distance", title: "Middle Distance", artist: "Mara Quinn", year: 2024, glow: "#e6b48a" },
];
export const byId = Object.fromEntries(albums.map((a) => [a.id, a]));
export const cover = (id) => `/img/covers/${id}.webp`;

// What plays in the drawings, in turn.
export const playing = [
  { song: "Opening", album: "harbor-lights", time: "3:48" },
  { song: "Paper Lanterns", album: "bloom-season", time: "4:12" },
  { song: "Last Crossing", album: "night-ferry", time: "5:03" },
  { song: "Coming Home", album: "afterglow-arcade", time: "3:27" },
];

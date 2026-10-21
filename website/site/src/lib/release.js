// Read when the site is built: the versions and file names from public/latest.json,
// public/latest-macos.json, and public/latest-android.json (the iPhone app's from
// public/sidestore.json), and each file's size from the release folder when it is there.
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { pick } from "./assets.js";

// The build runs in website/site, so website/ is one up.
const root = resolve(process.cwd(), "..");
const read = (name) => JSON.parse(readFileSync(join(root, "public", name), "utf8"));

// Sizes of the published files, for when the release folder is not on this computer.
const known = {
  "Needle-Setup-1.6.3.exe": 53322082,
  "Needle-1.6.3-windows-x64.zip": 71228138,
  "Needle-1.6.4-macos.dmg": 53923494,
  "Needle-1.7.0-macos.dmg": 52257872,
  "needle_1.6.3_amd64.deb": 52e6,
  "needle-1.6.3-1.x86_64.rpm": 71e6,
  "Needle-1.7.0-android-arm64-release.apk": 35846232,
  "Needle-1.7.0-android-x86_64-release.apk": 36649033,
};

const readIfThere = (name) => (existsSync(join(root, "public", name)) ? read(name) : null);

function findSize(name) {
  const dist = join(root, "..", "dist");
  const look = (dir, depth) => {
    if (!existsSync(dir)) return 0;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isFile() && entry.name === name) return statSync(path).size;
      if (entry.isDirectory() && depth > 0) {
        const size = look(path, depth - 1);
        if (size) return size;
      }
    }
    return 0;
  };
  return look(dist, 2) || known[name] || 0;
}

export function release() {
  const { files, version, macVersion, androidVersion } = pick(read("latest.json"), read("latest-macos.json"), readIfThere("latest-android.json"));
  const sizes = {};
  for (const [kind, name] of Object.entries(files)) {
    const bytes = findSize(name);
    if (bytes) sizes[kind] = `${Math.round(bytes / 1e6)} MB`;
  }
  // The iPhone app, for SideStore and AltStore: its newest version is first.
  const iphone = readIfThere("sidestore.json")?.apps?.[0]?.versions?.[0];
  if (iphone) {
    files.ipa = iphone.downloadURL.split("/").pop();
    sizes.ipa = `${Math.round(iphone.size / 1e6)} MB`;
  }
  return { files, sizes, version, macVersion, androidVersion, iphoneVersion: iphone?.version ?? "" };
}

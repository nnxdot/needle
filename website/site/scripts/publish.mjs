// Copies the built site from dist/ into ../public, which Wrangler serves.
// Only files the build makes are written. latest.json, latest-macos.json, install-mac.sh and
// the icons stay as they are in ../public: Needle's updaters and the Mac installer read them.
import { cpSync, existsSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";

const dist = new URL("../dist/", import.meta.url).pathname.replace(/^\/(\w:)/, "$1");
const out = new URL("../../public/", import.meta.url).pathname.replace(/^\/(\w:)/, "$1");
const kept = ["latest.json", "latest-macos.json", "install-mac.sh"];

// Old hashed files from an earlier build go first.
const assets = join(out, "_astro");
if (existsSync(assets)) rmSync(assets, { recursive: true });

for (const name of readdirSync(dist)) {
  if (kept.includes(name)) throw new Error(`The build must not make ${name}; it lives in public/ only.`);
  cpSync(join(dist, name), join(out, name), { recursive: true });
}
console.log(`Copied the site into ${out}`);

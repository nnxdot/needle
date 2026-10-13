// Which file is which, by name. The same names Needle's updaters and install-mac.sh read from
// latest.json and latest-macos.json. Used when the site is built, and again in the browser,
// so the buttons follow latest.json even before the site is built again.
export const kinds = {
  setup: /^Needle-Setup-[\d.]+\.exe$/,
  zip: /^Needle-[\d.]+-windows-x64\.zip$/,
  dmg: /^Needle-[\d.]+-macos\.dmg$/,
  deb: /^needle_[\d.]+_amd64\.deb$/,
  rpm: /^needle-[\d.]+-1\.x86_64\.rpm$/,
  apkArm: /^Needle-[\d.]+-android-arm64-release\.apk$/,
  apkX86: /^Needle-[\d.]+-android-x86_64-release\.apk$/,
};

/** { kind: file name } and the versions, from latest.json and latest-macos.json. */
export function pick(latest, latestMac) {
  const files = {};
  for (const asset of latest?.assets ?? []) {
    for (const [kind, test] of Object.entries(kinds)) if (test.test(asset.name)) files[kind] = asset.name;
  }
  // The Mac app has its own, newer, releases.
  for (const asset of latestMac?.assets ?? []) if (kinds.dmg.test(asset.name)) files.dmg = asset.name;
  const version = (latest?.tag_name ?? "").replace(/^v/, "");
  const macVersion = files.dmg ? files.dmg.replace(/^Needle-/, "").replace(/-macos\.dmg$/, "") : version;
  return { files, version, macVersion };
}

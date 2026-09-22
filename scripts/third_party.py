"""Regenerate THIRD-PARTY-NOTICES.md and third-party/licenses.txt from Cargo.lock.

Run from the repository root: python scripts/third_party.py
Also copies the original .crate archive of every MPL-licensed dependency into
third-party/sources/, as the MPL asks for source availability.
"""
import glob
import json
import os
import shutil
import subprocess

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
cargo_home = os.environ.get("CARGO_HOME", os.path.join(os.path.expanduser("~"), ".cargo"))
meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"],
    cwd=root))

packages = sorted(
    (p for p in meta["packages"]
     if (p.get("source") and p["source"].startswith("registry")) or "third-party" in p["manifest_path"]),
    key=lambda p: (p["name"].lower(), p["version"]))

def registry_dir(name, version):
    hits = glob.glob(os.path.join(cargo_home, "registry", "src", "*", f"{name}-{version}"))
    return hits[0] if hits else None

def crate_archive(name, version):
    hits = glob.glob(os.path.join(cargo_home, "registry", "cache", "*", f"{name}-{version}.crate"))
    return hits[0] if hits else None

rows, texts, mpl = [], [], []
for p in packages:
    name, version = p["name"], p["version"]
    license = p.get("license") or ("See " + p["license_file"] if p.get("license_file") else "Unspecified")
    patched = "third-party" in p["manifest_path"]
    note = " (patched copy in third-party/patched)" if patched else ""
    rows.append(f"| {name}-{version}{note} | {license} | [crates.io](https://crates.io/crates/{name}/{version}) |")
    folder = os.path.dirname(p["manifest_path"]) or registry_dir(name, version)
    if folder and os.path.isdir(folder):
        for entry in sorted(os.listdir(folder)):
            upper = entry.upper()
            if upper.startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE", "COPYRIGHT")) and os.path.isfile(os.path.join(folder, entry)):
                with open(os.path.join(folder, entry), encoding="utf-8", errors="replace") as f:
                    texts.append("=" * 72 + f"\n{name}-{version} / {entry}\n" + "=" * 72 + "\n" + f.read().strip() + "\n\n")
    if "MPL" in license:
        mpl.append((name, version))

header = """# Third-party notices

Needle uses the packages below. This conservative Cargo inventory includes runtime, build, and development dependencies for the Windows resolution. Each retains its own license. Exact versions are locked in Cargo.lock. Regenerate this file with `python scripts/third_party.py`.

Bundled license texts are in `third-party/licenses.txt`. Original, unmodified MPL-2.0 crate source archives are in `third-party/sources/`; they can also be obtained from each linked crates.io release. Needle's application source does not modify these dependencies, with one exception: `third-party/patched/opus-decoder-0.1.1/` is the published opus-decoder 0.1.1 crate with its O(n²) MDCT DFT replaced by a `rustfft` transform (`src/celt/kiss_fft.rs`), its tests and dev-dependencies removed, and upstream license files added. Cargo uses it through `[patch.crates-io]`.

## Components that are not Cargo packages

- **ONNX Runtime** (MIT License, Copyright (c) Microsoft Corporation) is linked into Needle through the `ort` crate, which downloads Microsoft's prebuilt runtime at build time. It runs the stem-separation model.
- **HT-Demucs** stem-separation model (MIT License, Copyright (c) Meta Platforms, Inc. and affiliates; Rouard, Massa and Défossez, "Hybrid Transformers for Music Source Separation", ICASSP 2023), in the ONNX export published by StemSplit at huggingface.co/StemSplitio/htdemucs-onnx (MIT). It is not included in the package; Needle downloads it when you first split a song.
- **Online services** used only when you turn them on: MusicBrainz, the Cover Art Archive, AcoustID, LRCLIB (lyrics), Wikidata and Wikimedia Commons (artist photos; each image keeps its own Commons license), Last.fm, and ListenBrainz.

| Package | License | Source release |
|---|---|---|
"""
with open(os.path.join(root, "THIRD-PARTY-NOTICES.md"), "w", encoding="utf-8", newline="\n") as f:
    f.write(header + "\n".join(rows) + "\n")
os.makedirs(os.path.join(root, "third-party", "sources"), exist_ok=True)
with open(os.path.join(root, "third-party", "licenses.txt"), "w", encoding="utf-8", newline="\n") as f:
    f.write("".join(texts))
wanted = set()
for name, version in mpl:
    archive = crate_archive(name, version)
    if archive:
        target = os.path.join(root, "third-party", "sources", os.path.basename(archive))
        wanted.add(os.path.basename(archive))
        if not os.path.exists(target):
            shutil.copy2(archive, target)
    else:
        print(f"warning: no .crate archive found for MPL package {name}-{version}")
for existing in os.listdir(os.path.join(root, "third-party", "sources")):
    if existing.endswith(".crate") and existing not in wanted:
        os.remove(os.path.join(root, "third-party", "sources", existing))
print(f"{len(rows)} packages, {len(texts)} license texts, {len(mpl)} MPL source archives")

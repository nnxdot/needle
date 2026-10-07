# Third-party notices

Needle uses the packages below. This conservative Cargo inventory includes runtime, build, and development dependencies for the Windows resolution. Each retains its own license. Exact versions are locked in Cargo.lock. Regenerate this file with `python scripts/third_party.py`.

Bundled license texts are in `third-party/licenses.txt`. Original, unmodified MPL-2.0 crate source archives are in `third-party/sources/`; they can also be obtained from each linked crates.io release. Needle's application source does not modify these dependencies, with three exceptions. `third-party/patched/gpui-0.2.2/` is the published gpui 0.2.2 crate (Apache-2.0) with two blend-state lines in `src/platform/windows/directx_renderer.rs` changed so soft edges blend correctly on see-through windows, two float literals typed in `src/taffy.rs`, and `src/elements/text.rs` reusing a measured text layout only when the width it was cut to fit is the same (a layout cut to 0 px during flex sizing otherwise stayed an ellipsis), and `src/platform/linux/wayland/client.rs` accepting version 1 of `xdg_wm_base` (WSLg offers no later one), and the Linux window code (`src/platform/linux/wayland/window.rs`, `src/platform/linux/x11/window.rs`) not starting an interactive resize for a window that is not resizable and holding Wayland configures to the window's minimum size; it also has an optional frame timing log for measuring stutters (`NEEDLE_FRAME_LOG`, in `src/window.rs`, with the reasons for new frames noted in `src/app/context.rs` and `src/elements/animation.rs`), and `DispatchEventResult` made public. `third-party/patched/gpui-component-0.5.1/` is the published gpui-component 0.5.1 crate (Apache-2.0) with its text input asking for a new frame only when something it keeps from painting changed (`src/input/element.rs`, `src/input/state.rs`), where it used to ask on every paint, and its tests removed; its Linux window border now uses the current drawable viewport for resize hit testing (rather than the saved restore bounds) and excludes tiled edges (`src/window_border.rs`), with resize regression tests run by `crates/needle/tests/window_border.rs`. `third-party/patched/opus-decoder-0.1.1/` is the published opus-decoder 0.1.1 crate with its O(n²) MDCT DFT replaced by a `rustfft` transform (`src/celt/kiss_fft.rs`), its tests and dev-dependencies removed, and upstream license files added. Cargo uses all three through `[patch.crates-io]`.

## Components that are not Cargo packages

- **ONNX Runtime** (MIT License, Copyright (c) Microsoft Corporation) is linked into Needle through the `ort` crate, which downloads Microsoft's prebuilt runtime at build time. It runs the stem-separation model.
- **HT-Demucs** stem-separation model (MIT License, Copyright (c) Meta Platforms, Inc. and affiliates; Rouard, Massa and Défossez, "Hybrid Transformers for Music Source Separation", ICASSP 2023), in the ONNX export published by StemSplit at huggingface.co/StemSplitio/htdemucs-onnx (MIT). It is not included in the package; Needle downloads it when you first split a song.
- **Fraunces** typeface (72pt Soft SemiBold and Bold; SIL Open Font License 1.1, Copyright 2018 The Fraunces Project Authors, github.com/undercasetype/Fraunces) is built into Needle for titles. Its license text is at the end of `third-party/licenses.txt`.
- **FFmpeg** 7.1.1 (GNU LGPL 2.1 or later; Copyright the FFmpeg developers, ffmpeg.org) is shipped as `needle-ffmpeg.exe`, a separate program Needle runs to decode Dolby Digital and Dolby Digital Plus (Atmos) music. It is built with only the MP4 reader, the AC-3 and E-AC-3 decoders, and audio resampling, and with no GPL or version-3 parts, by `scripts/build-ffmpeg.sh` from the unmodified release `ffmpeg-7.1.1.tar.xz` (SHA-256 733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1, from ffmpeg.org/releases). Its license is `third-party/ffmpeg/COPYING.LGPLv2.1`; the source release is also published with each Needle release. Dolby, Dolby Atmos, and Dolby Digital Plus are trademarks of Dolby Laboratories.
- **Online services** used only when you turn them on: MusicBrainz, the Cover Art Archive, AcoustID, LRCLIB (lyrics), Wikidata and Wikimedia Commons (artist photos; each image keeps its own Commons license), Last.fm, and ListenBrainz.

| Package | License | Source release |
|---|---|---|
| adler2-2.0.1 | 0BSD OR MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/adler2/2.0.1) |
| aead-0.5.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/aead/0.5.2) |
| ahash-0.8.12 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ahash/0.8.12) |
| aho-corasick-1.1.5 | Unlicense OR MIT | [crates.io](https://crates.io/crates/aho-corasick/1.1.5) |
| aligned-0.4.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/aligned/0.4.3) |
| aligned-vec-0.6.4 | MIT | [crates.io](https://crates.io/crates/aligned-vec/0.6.4) |
| anstream-1.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anstream/1.0.0) |
| anstyle-1.0.14 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anstyle/1.0.14) |
| anstyle-parse-1.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anstyle-parse/1.0.0) |
| anstyle-query-1.1.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anstyle-query/1.1.5) |
| anstyle-wincon-3.0.11 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anstyle-wincon/3.0.11) |
| anyhow-1.0.104 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/anyhow/1.0.104) |
| ar_archive_writer-0.5.3 | Apache-2.0 WITH LLVM-exception | [crates.io](https://crates.io/crates/ar_archive_writer/0.5.3) |
| arc-swap-1.9.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/arc-swap/1.9.2) |
| arg_enum_proc_macro-0.3.4 | MIT | [crates.io](https://crates.io/crates/arg_enum_proc_macro/0.3.4) |
| argon2-0.5.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/argon2/0.5.3) |
| arrayref-0.3.9 | BSD-2-Clause | [crates.io](https://crates.io/crates/arrayref/0.3.9) |
| arrayvec-0.7.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/arrayvec/0.7.8) |
| as-slice-0.2.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/as-slice/0.2.1) |
| ash-0.38.0+1.3.281 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ash/0.38.0+1.3.281) |
| ash-window-0.13.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ash-window/0.13.0) |
| async-channel-1.9.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-channel/1.9.0) |
| async-channel-2.5.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-channel/2.5.0) |
| async-compression-0.4.48 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/async-compression/0.4.48) |
| async-executor-1.14.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-executor/1.14.0) |
| async-fs-2.2.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-fs/2.2.0) |
| async-global-executor-2.4.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-global-executor/2.4.1) |
| async-io-2.6.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-io/2.6.0) |
| async-lock-3.4.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-lock/3.4.2) |
| async-net-2.0.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-net/2.0.0) |
| async-process-2.5.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-process/2.5.0) |
| async-std-1.13.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-std/1.13.2) |
| async-task-4.7.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/async-task/4.7.1) |
| async_zip-0.0.17 | MIT | [crates.io](https://crates.io/crates/async_zip/0.0.17) |
| atomic-0.5.3 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/atomic/0.5.3) |
| atomic-waker-1.1.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/atomic-waker/1.1.2) |
| autocfg-1.5.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/autocfg/1.5.1) |
| av-scenechange-0.14.1 | MIT | [crates.io](https://crates.io/crates/av-scenechange/0.14.1) |
| av1-grain-0.2.5 | BSD-2-Clause | [crates.io](https://crates.io/crates/av1-grain/0.2.5) |
| avif-serialize-0.8.9 | BSD-3-Clause | [crates.io](https://crates.io/crates/avif-serialize/0.8.9) |
| base62-2.2.6 | MIT | [crates.io](https://crates.io/crates/base62/2.2.6) |
| base64-0.22.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/base64/0.22.1) |
| base64-0.23.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/base64/0.23.1) |
| base64ct-1.8.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/base64ct/1.8.3) |
| bit-set-0.8.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/bit-set/0.8.0) |
| bit-vec-0.8.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/bit-vec/0.8.0) |
| bit_field-0.10.3 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/bit_field/0.10.3) |
| bitflags-1.3.2 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/bitflags/1.3.2) |
| bitflags-2.13.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/bitflags/2.13.2) |
| bitstream-io-4.10.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/bitstream-io/4.10.0) |
| blade-graphics-0.7.1 | MIT | [crates.io](https://crates.io/crates/blade-graphics/0.7.1) |
| blade-macros-0.3.0 | MIT | [crates.io](https://crates.io/crates/blade-macros/0.3.0) |
| blade-util-0.3.0 | MIT | [crates.io](https://crates.io/crates/blade-util/0.3.0) |
| blake2-0.10.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/blake2/0.10.6) |
| blake3-1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | [crates.io](https://crates.io/crates/blake3/1.8.7) |
| block-buffer-0.10.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/block-buffer/0.10.4) |
| block-buffer-0.12.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/block-buffer/0.12.1) |
| block-padding-0.3.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/block-padding/0.3.3) |
| blocking-1.7.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/blocking/1.7.0) |
| bstr-1.13.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/bstr/1.13.1) |
| built-0.8.1 | MIT | [crates.io](https://crates.io/crates/built/0.8.1) |
| bumpalo-3.20.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/bumpalo/3.20.3) |
| bytemuck-1.25.2 | Zlib OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/bytemuck/1.25.2) |
| bytemuck_derive-1.12.1 | Zlib OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/bytemuck_derive/1.12.1) |
| byteorder-1.5.0 | Unlicense OR MIT | [crates.io](https://crates.io/crates/byteorder/1.5.0) |
| byteorder-lite-0.1.0 | Unlicense OR MIT | [crates.io](https://crates.io/crates/byteorder-lite/0.1.0) |
| bytes-1.12.1 | MIT | [crates.io](https://crates.io/crates/bytes/1.12.1) |
| cc-1.4.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/cc/1.4.7) |
| cfg-if-1.0.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/cfg-if/1.0.5) |
| cfg_aliases-0.2.2 | MIT | [crates.io](https://crates.io/crates/cfg_aliases/0.2.2) |
| chacha20-0.10.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/chacha20/0.10.2) |
| chacha20-0.9.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/chacha20/0.9.1) |
| chacha20poly1305-0.10.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/chacha20poly1305/0.10.1) |
| chrono-0.4.45 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/chrono/0.4.45) |
| cipher-0.4.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/cipher/0.4.4) |
| clap-4.6.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/clap/4.6.7) |
| clap_builder-4.6.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/clap_builder/4.6.7) |
| clap_derive-4.6.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/clap_derive/4.6.7) |
| clap_lex-1.1.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/clap_lex/1.1.1) |
| codespan-reporting-0.12.0 | Apache-2.0 | [crates.io](https://crates.io/crates/codespan-reporting/0.12.0) |
| color_quant-1.1.0 | MIT | [crates.io](https://crates.io/crates/color_quant/1.1.0) |
| colorchoice-1.0.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/colorchoice/1.0.5) |
| compression-codecs-0.4.43 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/compression-codecs/0.4.43) |
| compression-core-0.4.33 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/compression-core/0.4.33) |
| concurrent-queue-2.5.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/concurrent-queue/2.5.0) |
| const-oid-0.10.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/const-oid/0.10.2) |
| const-random-0.1.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/const-random/0.1.18) |
| const-random-macro-0.1.16 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/const-random-macro/0.1.16) |
| constant_time_eq-0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 | [crates.io](https://crates.io/crates/constant_time_eq/0.4.2) |
| convert_case-0.4.0 | MIT | [crates.io](https://crates.io/crates/convert_case/0.4.0) |
| core_detect-1.0.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/core_detect/1.0.0) |
| core_maths-0.1.1 | MIT | [crates.io](https://crates.io/crates/core_maths/0.1.1) |
| cpal-0.16.0 | Apache-2.0 | [crates.io](https://crates.io/crates/cpal/0.16.0) |
| cpufeatures-0.2.17 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/cpufeatures/0.2.17) |
| cpufeatures-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/cpufeatures/0.3.1) |
| crc32fast-1.5.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crc32fast/1.5.2) |
| crossbeam-channel-0.5.17 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crossbeam-channel/0.5.17) |
| crossbeam-deque-0.8.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crossbeam-deque/0.8.8) |
| crossbeam-epoch-0.9.21 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crossbeam-epoch/0.9.21) |
| crossbeam-queue-0.3.14 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crossbeam-queue/0.3.14) |
| crossbeam-utils-0.8.23 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crossbeam-utils/0.8.23) |
| crunchy-0.2.4 | MIT | [crates.io](https://crates.io/crates/crunchy/0.2.4) |
| crypto-common-0.1.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crypto-common/0.1.7) |
| crypto-common-0.2.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/crypto-common/0.2.2) |
| ctor-0.4.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/ctor/0.4.3) |
| ctor-proc-macro-0.0.6 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/ctor-proc-macro/0.0.6) |
| dasp_frame-0.11.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dasp_frame/0.11.0) |
| dasp_sample-0.11.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dasp_sample/0.11.0) |
| data-encoding-2.11.1 | MIT | [crates.io](https://crates.io/crates/data-encoding/2.11.1) |
| data-url-0.3.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/data-url/0.3.2) |
| deflate64-0.1.12 | MIT | [crates.io](https://crates.io/crates/deflate64/0.1.12) |
| der-0.8.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/der/0.8.2) |
| deranged-0.5.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/deranged/0.5.8) |
| derive_more-0.99.20 | MIT | [crates.io](https://crates.io/crates/derive_more/0.99.20) |
| digest-0.10.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/digest/0.10.7) |
| digest-0.11.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/digest/0.11.3) |
| directories-6.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/directories/6.0.0) |
| dirs-4.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dirs/4.0.0) |
| dirs-6.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dirs/6.0.0) |
| dirs-sys-0.3.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dirs-sys/0.3.7) |
| dirs-sys-0.5.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dirs-sys/0.5.0) |
| displaydoc-0.2.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/displaydoc/0.2.7) |
| dtor-0.0.6 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/dtor/0.0.6) |
| dtor-proc-macro-0.0.5 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/dtor-proc-macro/0.0.5) |
| dunce-1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | [crates.io](https://crates.io/crates/dunce/1.0.5) |
| dyn-clone-1.0.20 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/dyn-clone/1.0.20) |
| ebur128-0.1.10 | MIT | [crates.io](https://crates.io/crates/ebur128/0.1.10) |
| either-1.18.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/either/1.18.0) |
| embed-resource-3.0.11 | MIT | [crates.io](https://crates.io/crates/embed-resource/3.0.11) |
| encoding_rs-0.8.41 | (Apache-2.0 OR MIT) AND BSD-3-Clause | [crates.io](https://crates.io/crates/encoding_rs/0.8.41) |
| enum-iterator-2.3.0 | 0BSD | [crates.io](https://crates.io/crates/enum-iterator/2.3.0) |
| enum-iterator-derive-1.5.0 | 0BSD | [crates.io](https://crates.io/crates/enum-iterator-derive/1.5.0) |
| equator-0.4.2 | MIT | [crates.io](https://crates.io/crates/equator/0.4.2) |
| equator-macro-0.4.2 | MIT | [crates.io](https://crates.io/crates/equator-macro/0.4.2) |
| equivalent-1.0.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/equivalent/1.0.2) |
| erased-serde-0.4.10 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/erased-serde/0.4.10) |
| errno-0.3.14 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/errno/0.3.14) |
| etagere-0.2.15 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/etagere/0.2.15) |
| euclid-0.22.14 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/euclid/0.22.14) |
| event-listener-2.5.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/event-listener/2.5.3) |
| event-listener-5.4.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/event-listener/5.4.2) |
| event-listener-strategy-0.5.4 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/event-listener-strategy/0.5.4) |
| exr-1.74.2 | BSD-3-Clause | [crates.io](https://crates.io/crates/exr/1.74.2) |
| extended-0.1.0 | MIT | [crates.io](https://crates.io/crates/extended/0.1.0) |
| fallible-iterator-0.3.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/fallible-iterator/0.3.0) |
| fallible-streaming-iterator-0.1.9 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/fallible-streaming-iterator/0.1.9) |
| fastrand-1.9.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/fastrand/1.9.0) |
| fastrand-2.5.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/fastrand/2.5.0) |
| fax-0.2.7 | MIT | [crates.io](https://crates.io/crates/fax/0.2.7) |
| fdeflate-0.3.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/fdeflate/0.3.7) |
| filetime-0.2.29 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/filetime/0.2.29) |
| find-msvc-tools-0.1.13 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/find-msvc-tools/0.1.13) |
| flate2-1.1.10 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/flate2/1.1.10) |
| float-cmp-0.9.0 | MIT | [crates.io](https://crates.io/crates/float-cmp/0.9.0) |
| float_next_after-1.0.0 | MIT | [crates.io](https://crates.io/crates/float_next_after/1.0.0) |
| fluent-uri-0.1.4 | MIT | [crates.io](https://crates.io/crates/fluent-uri/0.1.4) |
| flume-0.11.1 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/flume/0.11.1) |
| fnv-1.0.7 | Apache-2.0 / MIT | [crates.io](https://crates.io/crates/fnv/1.0.7) |
| foldhash-0.1.5 | Zlib | [crates.io](https://crates.io/crates/foldhash/0.1.5) |
| fontdb-0.23.0 | MIT | [crates.io](https://crates.io/crates/fontdb/0.23.0) |
| form_urlencoded-1.2.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/form_urlencoded/1.2.2) |
| futf-0.1.5 | MIT / Apache-2.0 | [crates.io](https://crates.io/crates/futf/0.1.5) |
| futures-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures/0.3.34) |
| futures-channel-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-channel/0.3.34) |
| futures-core-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-core/0.3.34) |
| futures-executor-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-executor/0.3.34) |
| futures-io-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-io/0.3.34) |
| futures-lite-1.13.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/futures-lite/1.13.0) |
| futures-lite-2.6.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/futures-lite/2.6.1) |
| futures-macro-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-macro/0.3.34) |
| futures-sink-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-sink/0.3.34) |
| futures-task-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-task/0.3.34) |
| futures-util-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/futures-util/0.3.34) |
| generic-array-0.14.7 | MIT | [crates.io](https://crates.io/crates/generic-array/0.14.7) |
| getrandom-0.2.17 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/getrandom/0.2.17) |
| getrandom-0.3.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/getrandom/0.3.4) |
| getrandom-0.4.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/getrandom/0.4.3) |
| gif-0.14.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/gif/0.14.2) |
| glob-0.3.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/glob/0.3.4) |
| globset-0.4.20 | Unlicense OR MIT | [crates.io](https://crates.io/crates/globset/0.4.20) |
| globwalk-0.8.1 | MIT | [crates.io](https://crates.io/crates/globwalk/0.8.1) |
| gpu-alloc-0.6.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/gpu-alloc/0.6.2) |
| gpu-alloc-ash-0.7.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/gpu-alloc-ash/0.7.1) |
| gpu-alloc-types-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/gpu-alloc-types/0.3.1) |
| gpui-0.2.2 (patched copy in third-party/patched) | Apache-2.0 | [crates.io](https://crates.io/crates/gpui/0.2.2) |
| gpui-component-0.5.1 (patched copy in third-party/patched) | Apache-2.0 | [crates.io](https://crates.io/crates/gpui-component/0.5.1) |
| gpui-component-assets-0.5.1 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui-component-assets/0.5.1) |
| gpui-component-macros-0.5.1 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui-component-macros/0.5.1) |
| gpui-macros-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui-macros/0.2.2) |
| gpui_collections-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_collections/0.2.2) |
| gpui_derive_refineable-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_derive_refineable/0.2.2) |
| gpui_http_client-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_http_client/0.2.2) |
| gpui_perf-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_perf/0.2.2) |
| gpui_refineable-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_refineable/0.2.2) |
| gpui_semantic_version-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_semantic_version/0.2.2) |
| gpui_sum_tree-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_sum_tree/0.2.2) |
| gpui_util-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_util/0.2.2) |
| gpui_util_macros-0.2.2 | Apache-2.0 | [crates.io](https://crates.io/crates/gpui_util_macros/0.2.2) |
| grid-0.18.0 | MIT | [crates.io](https://crates.io/crates/grid/0.18.0) |
| h2-0.4.19 | MIT | [crates.io](https://crates.io/crates/h2/0.4.19) |
| half-2.7.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/half/2.7.1) |
| hashbrown-0.15.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/hashbrown/0.15.5) |
| hashbrown-0.17.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/hashbrown/0.17.1) |
| hashlink-0.10.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/hashlink/0.10.0) |
| heck-0.5.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/heck/0.5.0) |
| hexf-parse-0.2.1 | CC0-1.0 | [crates.io](https://crates.io/crates/hexf-parse/0.2.1) |
| hidden-trait-0.1.2 | MIT | [crates.io](https://crates.io/crates/hidden-trait/0.1.2) |
| hmac-sha256-1.1.14 | ISC | [crates.io](https://crates.io/crates/hmac-sha256/1.1.14) |
| home-0.5.12 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/home/0.5.12) |
| hound-3.5.1 | Apache-2.0 | [crates.io](https://crates.io/crates/hound/3.5.1) |
| html5ever-0.27.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/html5ever/0.27.0) |
| http-1.5.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/http/1.5.0) |
| http-body-1.1.0 | MIT | [crates.io](https://crates.io/crates/http-body/1.1.0) |
| http-body-util-0.1.5 | MIT | [crates.io](https://crates.io/crates/http-body-util/0.1.5) |
| httparse-1.10.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/httparse/1.10.1) |
| hybrid-array-0.4.15 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/hybrid-array/0.4.15) |
| hyper-1.11.1 | MIT | [crates.io](https://crates.io/crates/hyper/1.11.1) |
| hyper-rustls-0.27.10 | Apache-2.0 OR ISC OR MIT | [crates.io](https://crates.io/crates/hyper-rustls/0.27.10) |
| hyper-util-0.1.20 | MIT | [crates.io](https://crates.io/crates/hyper-util/0.1.20) |
| icu_collections-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_collections/2.3.0) |
| icu_locale_core-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_locale_core/2.3.0) |
| icu_normalizer-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_normalizer/2.3.0) |
| icu_normalizer_data-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_normalizer_data/2.3.0) |
| icu_properties-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_properties/2.3.0) |
| icu_properties_data-2.3.0 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_properties_data/2.3.0) |
| icu_provider-2.3.1 | Unicode-3.0 | [crates.io](https://crates.io/crates/icu_provider/2.3.1) |
| idna-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/idna/1.1.0) |
| idna_adapter-1.2.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/idna_adapter/1.2.2) |
| ignore-0.4.33 | Unlicense OR MIT | [crates.io](https://crates.io/crates/ignore/0.4.33) |
| image-0.25.10 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/image/0.25.10) |
| image-webp-0.2.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/image-webp/0.2.4) |
| imagesize-0.13.0 | MIT | [crates.io](https://crates.io/crates/imagesize/0.13.0) |
| imgref-1.12.3 | CC0-1.0 OR Apache-2.0 | [crates.io](https://crates.io/crates/imgref/1.12.3) |
| indexmap-2.14.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/indexmap/2.14.2) |
| inout-0.1.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/inout/0.1.4) |
| instant-0.1.13 | BSD-3-Clause | [crates.io](https://crates.io/crates/instant/0.1.13) |
| inventory-0.3.24 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/inventory/0.3.24) |
| ipnet-2.12.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ipnet/2.12.2) |
| is_terminal_polyfill-1.70.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/is_terminal_polyfill/1.70.2) |
| itertools-0.11.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/itertools/0.11.0) |
| itertools-0.13.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/itertools/0.13.0) |
| itertools-0.14.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/itertools/0.14.0) |
| itoa-1.0.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/itoa/1.0.18) |
| jobserver-0.1.35 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/jobserver/0.1.35) |
| keyring-3.6.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/keyring/3.6.3) |
| kurbo-0.11.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/kurbo/0.11.3) |
| kv-log-macro-1.0.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/kv-log-macro/1.0.7) |
| lazy_static-1.5.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lazy_static/1.5.0) |
| lebe-0.5.3 | BSD-3-Clause | [crates.io](https://crates.io/crates/lebe/0.5.3) |
| libc-0.2.189 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/libc/0.2.189) |
| libloading-0.8.9 | ISC | [crates.io](https://crates.io/crates/libloading/0.8.9) |
| libm-0.2.16 | MIT | [crates.io](https://crates.io/crates/libm/0.2.16) |
| libsqlite3-sys-0.35.0 | MIT | [crates.io](https://crates.io/crates/libsqlite3-sys/0.35.0) |
| litemap-0.8.3 | Unicode-3.0 | [crates.io](https://crates.io/crates/litemap/0.8.3) |
| lock_api-0.4.14 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lock_api/0.4.14) |
| lofty-0.24.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lofty/0.24.0) |
| lofty_attr-0.12.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lofty_attr/0.12.0) |
| log-0.4.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/log/0.4.34) |
| loop9-0.1.5 | MIT | [crates.io](https://crates.io/crates/loop9/0.1.5) |
| lru-slab-0.1.3 | MIT OR Apache-2.0 OR Zlib | [crates.io](https://crates.io/crates/lru-slab/0.1.3) |
| lsp-types-0.97.0 | MIT | [crates.io](https://crates.io/crates/lsp-types/0.97.0) |
| lyon-1.0.19 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lyon/1.0.19) |
| lyon_algorithms-1.0.21 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lyon_algorithms/1.0.21) |
| lyon_geom-1.0.19 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lyon_geom/1.0.19) |
| lyon_path-1.0.19 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lyon_path/1.0.19) |
| lyon_tessellation-1.0.22 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/lyon_tessellation/1.0.22) |
| lzma-rust2-0.15.8 | Apache-2.0 | [crates.io](https://crates.io/crates/lzma-rust2/0.15.8) |
| mac-0.1.1 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/mac/0.1.1) |
| markdown-1.0.0 | MIT | [crates.io](https://crates.io/crates/markdown/1.0.0) |
| markup5ever-0.12.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/markup5ever/0.12.1) |
| markup5ever_rcdom-0.3.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/markup5ever_rcdom/0.3.0) |
| matrixmultiply-0.3.11 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/matrixmultiply/0.3.11) |
| maybe-rayon-0.1.1 | MIT | [crates.io](https://crates.io/crates/maybe-rayon/0.1.1) |
| md5-0.7.0 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/md5/0.7.0) |
| memchr-2.8.3 | Unlicense OR MIT | [crates.io](https://crates.io/crates/memchr/2.8.3) |
| memmap2-0.9.11 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/memmap2/0.9.11) |
| mime-0.3.17 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/mime/0.3.17) |
| mime_guess-2.0.5 | MIT | [crates.io](https://crates.io/crates/mime_guess/2.0.5) |
| miniz_oxide-0.8.9 | MIT OR Zlib OR Apache-2.0 | [crates.io](https://crates.io/crates/miniz_oxide/0.8.9) |
| miniz_oxide-0.9.1 | MIT OR Zlib OR Apache-2.0 | [crates.io](https://crates.io/crates/miniz_oxide/0.9.1) |
| mint-0.5.9 | MIT | [crates.io](https://crates.io/crates/mint/0.5.9) |
| mio-1.2.3 | MIT | [crates.io](https://crates.io/crates/mio/1.2.3) |
| moxcms-0.8.1 | BSD-3-Clause OR Apache-2.0 | [crates.io](https://crates.io/crates/moxcms/0.8.1) |
| multiversion-0.9.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/multiversion/0.9.0) |
| multiversion-macros-0.9.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/multiversion-macros/0.9.0) |
| multiversion_no_op-1.0.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/multiversion_no_op/1.0.0) |
| naga-25.0.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/naga/25.0.1) |
| nanorand-0.7.0 | Zlib | [crates.io](https://crates.io/crates/nanorand/0.7.0) |
| native-tls-0.2.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/native-tls/0.2.18) |
| ndarray-0.17.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ndarray/0.17.2) |
| new_debug_unreachable-1.0.6 | MIT | [crates.io](https://crates.io/crates/new_debug_unreachable/1.0.6) |
| no_std_io2-0.9.4 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/no_std_io2/0.9.4) |
| nom-8.0.0 | MIT | [crates.io](https://crates.io/crates/nom/8.0.0) |
| noop_proc_macro-0.3.0 | MIT | [crates.io](https://crates.io/crates/noop_proc_macro/0.3.0) |
| normpath-1.5.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/normpath/1.5.2) |
| notify-7.0.0 | CC0-1.0 | [crates.io](https://crates.io/crates/notify/7.0.0) |
| notify-8.2.0 | CC0-1.0 | [crates.io](https://crates.io/crates/notify/8.2.0) |
| notify-types-1.0.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/notify-types/1.0.1) |
| notify-types-2.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/notify-types/2.1.0) |
| ntapi-0.4.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/ntapi/0.4.3) |
| num-bigint-0.4.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-bigint/0.4.8) |
| num-complex-0.4.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-complex/0.4.6) |
| num-conv-0.2.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-conv/0.2.2) |
| num-derive-0.4.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-derive/0.4.2) |
| num-integer-0.1.47 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-integer/0.1.47) |
| num-rational-0.4.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-rational/0.4.2) |
| num-traits-0.2.19 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num-traits/0.2.19) |
| num_cpus-1.17.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/num_cpus/1.17.0) |
| object-0.39.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/object/0.39.1) |
| ogg_pager-0.7.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ogg_pager/0.7.2) |
| once_cell-1.21.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/once_cell/1.21.4) |
| once_cell_polyfill-1.70.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/once_cell_polyfill/1.70.2) |
| opaque-debug-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/opaque-debug/0.3.1) |
| option-ext-0.2.0 | MPL-2.0 | [crates.io](https://crates.io/crates/option-ext/0.2.0) |
| opus-decoder-0.1.1 (patched copy in third-party/patched) | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/opus-decoder/0.1.1) |
| ort-2.0.0-rc.13 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ort/2.0.0-rc.13) |
| ort-sys-2.0.0-rc.13 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ort-sys/2.0.0-rc.13) |
| parking-2.2.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/parking/2.2.1) |
| parking_lot-0.12.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/parking_lot/0.12.5) |
| parking_lot_core-0.9.12 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/parking_lot_core/0.9.12) |
| password-hash-0.5.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/password-hash/0.5.0) |
| paste-1.0.15 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/paste/1.0.15) |
| pastey-0.1.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/pastey/0.1.1) |
| pem-rfc7468-1.0.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/pem-rfc7468/1.0.0) |
| percent-encoding-2.3.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/percent-encoding/2.3.2) |
| phf-0.11.3 | MIT | [crates.io](https://crates.io/crates/phf/0.11.3) |
| phf_codegen-0.11.3 | MIT | [crates.io](https://crates.io/crates/phf_codegen/0.11.3) |
| phf_generator-0.11.3 | MIT | [crates.io](https://crates.io/crates/phf_generator/0.11.3) |
| phf_shared-0.11.3 | MIT | [crates.io](https://crates.io/crates/phf_shared/0.11.3) |
| pico-args-0.5.0 | MIT | [crates.io](https://crates.io/crates/pico-args/0.5.0) |
| pin-project-1.1.13 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/pin-project/1.1.13) |
| pin-project-internal-1.1.13 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/pin-project-internal/1.1.13) |
| pin-project-lite-0.2.17 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/pin-project-lite/0.2.17) |
| pin-utils-0.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/pin-utils/0.1.0) |
| piper-0.2.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/piper/0.2.5) |
| pkg-config-0.3.34 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/pkg-config/0.3.34) |
| plist-1.10.1 | MIT | [crates.io](https://crates.io/crates/plist/1.10.1) |
| png-0.17.16 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/png/0.17.16) |
| png-0.18.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/png/0.18.1) |
| polling-3.11.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/polling/3.11.0) |
| pollster-0.2.5 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/pollster/0.2.5) |
| poly1305-0.8.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/poly1305/0.8.0) |
| portable-atomic-1.15.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/portable-atomic/1.15.0) |
| postage-0.5.0 | MIT | [crates.io](https://crates.io/crates/postage/0.5.0) |
| potential_utf-0.1.6 | Unicode-3.0 | [crates.io](https://crates.io/crates/potential_utf/0.1.6) |
| powerfmt-0.2.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/powerfmt/0.2.0) |
| ppv-lite86-0.2.21 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ppv-lite86/0.2.21) |
| precomputed-hash-0.1.1 | MIT | [crates.io](https://crates.io/crates/precomputed-hash/0.1.1) |
| primal-check-0.3.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/primal-check/0.3.4) |
| proc-macro-error-attr2-2.0.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/proc-macro-error-attr2/2.0.0) |
| proc-macro-error2-2.0.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/proc-macro-error2/2.0.1) |
| proc-macro2-1.0.107 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/proc-macro2/1.0.107) |
| profiling-1.0.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/profiling/1.0.18) |
| profiling-procmacros-1.0.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/profiling-procmacros/1.0.18) |
| psm-0.1.32 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/psm/0.1.32) |
| pulp-0.22.3 | MIT | [crates.io](https://crates.io/crates/pulp/0.22.3) |
| pulp-wasm-simd-flag-0.1.1 | MIT | [crates.io](https://crates.io/crates/pulp-wasm-simd-flag/0.1.1) |
| pxfm-0.1.30 | BSD-3-Clause OR Apache-2.0 | [crates.io](https://crates.io/crates/pxfm/0.1.30) |
| qoi-0.4.1 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/qoi/0.4.1) |
| quick-error-2.0.1 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/quick-error/2.0.1) |
| quick-xml-0.42.0 | MIT | [crates.io](https://crates.io/crates/quick-xml/0.42.0) |
| quinn-0.11.12 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/quinn/0.11.12) |
| quinn-proto-0.11.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/quinn-proto/0.11.18) |
| quinn-udp-0.5.15 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/quinn-udp/0.5.15) |
| quote-1.0.47 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/quote/1.0.47) |
| rand-0.10.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand/0.10.3) |
| rand-0.8.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand/0.8.8) |
| rand-0.9.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand/0.9.5) |
| rand_chacha-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_chacha/0.3.1) |
| rand_chacha-0.9.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_chacha/0.9.0) |
| rand_core-0.10.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_core/0.10.1) |
| rand_core-0.6.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_core/0.6.4) |
| rand_core-0.9.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_core/0.9.5) |
| rand_pcg-0.10.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rand_pcg/0.10.2) |
| rav1e-0.8.1 | BSD-2-Clause | [crates.io](https://crates.io/crates/rav1e/0.8.1) |
| ravif-0.13.0 | BSD-3-Clause | [crates.io](https://crates.io/crates/ravif/0.13.0) |
| raw-cpuid-11.6.0 | MIT | [crates.io](https://crates.io/crates/raw-cpuid/11.6.0) |
| raw-window-handle-0.6.2 | MIT OR Apache-2.0 OR Zlib | [crates.io](https://crates.io/crates/raw-window-handle/0.6.2) |
| rawpointer-0.2.1 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/rawpointer/0.2.1) |
| rayon-1.12.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rayon/1.12.0) |
| rayon-core-1.13.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rayon-core/1.13.0) |
| realfft-3.5.0 | MIT | [crates.io](https://crates.io/crates/realfft/3.5.0) |
| reborrow-0.5.5 | MIT | [crates.io](https://crates.io/crates/reborrow/0.5.5) |
| ref-cast-1.0.27 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ref-cast/1.0.27) |
| ref-cast-impl-1.0.27 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ref-cast-impl/1.0.27) |
| regex-1.13.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/regex/1.13.1) |
| regex-automata-0.4.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/regex-automata/0.4.18) |
| regex-syntax-0.8.11 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/regex-syntax/0.8.11) |
| reqwest-0.12.28 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/reqwest/0.12.28) |
| resvg-0.45.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/resvg/0.45.1) |
| rfd-0.15.4 | MIT | [crates.io](https://crates.io/crates/rfd/0.15.4) |
| rgb-0.8.53 | MIT | [crates.io](https://crates.io/crates/rgb/0.8.53) |
| rhai-1.26.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rhai/1.26.1) |
| rhai_codegen-3.2.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rhai_codegen/3.2.0) |
| ring-0.17.14 | Apache-2.0 AND ISC | [crates.io](https://crates.io/crates/ring/0.17.14) |
| rodio-0.21.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rodio/0.21.1) |
| ropey-2.0.0-beta.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ropey/2.0.0-beta.1) |
| roxmltree-0.20.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/roxmltree/0.20.0) |
| rubato-0.16.2 | MIT | [crates.io](https://crates.io/crates/rubato/0.16.2) |
| rusqlite-0.37.0 | MIT | [crates.io](https://crates.io/crates/rusqlite/0.37.0) |
| rust-embed-8.12.0 | MIT | [crates.io](https://crates.io/crates/rust-embed/8.12.0) |
| rust-embed-impl-8.12.0 | MIT | [crates.io](https://crates.io/crates/rust-embed-impl/8.12.0) |
| rust-embed-utils-8.12.0 | MIT | [crates.io](https://crates.io/crates/rust-embed-utils/8.12.0) |
| rust-i18n-3.1.5 | MIT | [crates.io](https://crates.io/crates/rust-i18n/3.1.5) |
| rust-i18n-macro-3.1.5 | MIT | [crates.io](https://crates.io/crates/rust-i18n-macro/3.1.5) |
| rust-i18n-support-3.1.5 | MIT | [crates.io](https://crates.io/crates/rust-i18n-support/3.1.5) |
| rustc-hash-1.1.0 | Apache-2.0/MIT | [crates.io](https://crates.io/crates/rustc-hash/1.1.0) |
| rustc-hash-2.1.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/rustc-hash/2.1.3) |
| rustc_version-0.4.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rustc_version/0.4.1) |
| rustfft-6.4.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rustfft/6.4.1) |
| rustix-1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/rustix/1.1.5) |
| rustls-0.23.45 | Apache-2.0 OR ISC OR MIT | [crates.io](https://crates.io/crates/rustls/0.23.45) |
| rustls-native-certs-0.8.4 | Apache-2.0 OR ISC OR MIT | [crates.io](https://crates.io/crates/rustls-native-certs/0.8.4) |
| rustls-pemfile-2.2.0 | Apache-2.0 OR ISC OR MIT | [crates.io](https://crates.io/crates/rustls-pemfile/2.2.0) |
| rustls-pki-types-1.15.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rustls-pki-types/1.15.1) |
| rustls-webpki-0.103.15 | ISC | [crates.io](https://crates.io/crates/rustls-webpki/0.103.15) |
| rustversion-1.0.23 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/rustversion/1.0.23) |
| rusty-chromaprint-0.3.0 | MIT | [crates.io](https://crates.io/crates/rusty-chromaprint/0.3.0) |
| rustybuzz-0.20.1 | MIT | [crates.io](https://crates.io/crates/rustybuzz/0.20.1) |
| ryu-1.0.23 | Apache-2.0 OR BSL-1.0 | [crates.io](https://crates.io/crates/ryu/1.0.23) |
| same-file-1.0.6 | Unlicense/MIT | [crates.io](https://crates.io/crates/same-file/1.0.6) |
| schannel-0.1.29 | MIT | [crates.io](https://crates.io/crates/schannel/0.1.29) |
| schemars-1.2.2 | MIT | [crates.io](https://crates.io/crates/schemars/1.2.2) |
| schemars_derive-1.2.2 | MIT | [crates.io](https://crates.io/crates/schemars_derive/1.2.2) |
| scopeguard-1.2.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/scopeguard/1.2.0) |
| seahash-4.1.0 | MIT | [crates.io](https://crates.io/crates/seahash/4.1.0) |
| semver-1.0.28 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/semver/1.0.28) |
| serde-1.0.229 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde/1.0.229) |
| serde_core-1.0.229 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_core/1.0.229) |
| serde_derive-1.0.229 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_derive/1.0.229) |
| serde_derive_internals-0.30.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_derive_internals/0.30.0) |
| serde_fmt-1.1.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/serde_fmt/1.1.0) |
| serde_json-1.0.151 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_json/1.0.151) |
| serde_json_lenient-0.2.4 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/serde_json_lenient/0.2.4) |
| serde_repr-0.1.21 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_repr/0.1.21) |
| serde_spanned-0.6.9 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_spanned/0.6.9) |
| serde_spanned-1.1.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_spanned/1.1.1) |
| serde_urlencoded-0.7.1 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/serde_urlencoded/0.7.1) |
| serde_yaml-0.9.34+deprecated | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/serde_yaml/0.9.34+deprecated) |
| sha1_smol-1.0.1 | BSD-3-Clause | [crates.io](https://crates.io/crates/sha1_smol/1.0.1) |
| sha2-0.10.9 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/sha2/0.10.9) |
| sha2-0.11.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/sha2/0.11.0) |
| shellexpand-3.1.2 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/shellexpand/3.1.2) |
| shlex-1.3.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/shlex/1.3.0) |
| shlex-2.0.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/shlex/2.0.1) |
| simd-adler32-0.3.10 | MIT | [crates.io](https://crates.io/crates/simd-adler32/0.3.10) |
| simd_helpers-0.1.0 | MIT | [crates.io](https://crates.io/crates/simd_helpers/0.1.0) |
| simdutf8-0.1.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/simdutf8/0.1.5) |
| simplecss-0.2.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/simplecss/0.2.2) |
| siphasher-1.0.3 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/siphasher/1.0.3) |
| slab-0.4.12 | MIT | [crates.io](https://crates.io/crates/slab/0.4.12) |
| slotmap-1.1.1 | Zlib | [crates.io](https://crates.io/crates/slotmap/1.1.1) |
| smallvec-1.16.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/smallvec/1.16.1) |
| smartstring-1.0.1 | MPL-2.0+ | [crates.io](https://crates.io/crates/smartstring/1.0.1) |
| smol-2.0.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/smol/2.0.2) |
| socket2-0.6.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/socket2/0.6.5) |
| socks-0.3.4 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/socks/0.3.4) |
| spin-0.9.9 | MIT | [crates.io](https://crates.io/crates/spin/0.9.9) |
| spirv-0.3.0+sdk-1.3.268.0 | Apache-2.0 | [crates.io](https://crates.io/crates/spirv/0.3.0+sdk-1.3.268.0) |
| stable_deref_trait-1.2.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/stable_deref_trait/1.2.1) |
| stacker-0.1.25 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/stacker/0.1.25) |
| stacksafe-0.1.4 | Apache-2.0 | [crates.io](https://crates.io/crates/stacksafe/0.1.4) |
| stacksafe-macro-0.1.4 | Apache-2.0 | [crates.io](https://crates.io/crates/stacksafe-macro/0.1.4) |
| static_assertions-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/static_assertions/1.1.0) |
| str_indices-0.4.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/str_indices/0.4.4) |
| streaming-iterator-0.1.9 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/streaming-iterator/0.1.9) |
| strength_reduce-0.2.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/strength_reduce/0.2.4) |
| strict-num-0.1.1 | MIT | [crates.io](https://crates.io/crates/strict-num/0.1.1) |
| string_cache-0.8.9 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/string_cache/0.8.9) |
| string_cache_codegen-0.5.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/string_cache_codegen/0.5.4) |
| strsim-0.11.1 | MIT | [crates.io](https://crates.io/crates/strsim/0.11.1) |
| strum-0.26.3 | MIT | [crates.io](https://crates.io/crates/strum/0.26.3) |
| strum-0.27.2 | MIT | [crates.io](https://crates.io/crates/strum/0.27.2) |
| strum_macros-0.26.4 | MIT | [crates.io](https://crates.io/crates/strum_macros/0.26.4) |
| strum_macros-0.27.2 | MIT | [crates.io](https://crates.io/crates/strum_macros/0.27.2) |
| subtle-2.6.1 | BSD-3-Clause | [crates.io](https://crates.io/crates/subtle/2.6.1) |
| sval-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval/2.22.0) |
| sval_buffer-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_buffer/2.22.0) |
| sval_dynamic-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_dynamic/2.22.0) |
| sval_fmt-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_fmt/2.22.0) |
| sval_json-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_json/2.22.0) |
| sval_nested-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_nested/2.22.0) |
| sval_ref-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_ref/2.22.0) |
| sval_serde-2.22.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/sval_serde/2.22.0) |
| svg_fmt-0.4.5 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/svg_fmt/0.4.5) |
| svgtypes-0.15.3 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/svgtypes/0.15.3) |
| symphonia-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia/0.5.5) |
| symphonia-bundle-flac-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-bundle-flac/0.5.5) |
| symphonia-bundle-mp3-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-bundle-mp3/0.5.5) |
| symphonia-codec-aac-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-codec-aac/0.5.5) |
| symphonia-codec-adpcm-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-codec-adpcm/0.5.5) |
| symphonia-codec-alac-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-codec-alac/0.5.5) |
| symphonia-codec-pcm-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-codec-pcm/0.5.5) |
| symphonia-codec-vorbis-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-codec-vorbis/0.5.5) |
| symphonia-core-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-core/0.5.5) |
| symphonia-format-caf-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-format-caf/0.5.5) |
| symphonia-format-isomp4-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-format-isomp4/0.5.5) |
| symphonia-format-mkv-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-format-mkv/0.5.5) |
| symphonia-format-ogg-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-format-ogg/0.5.5) |
| symphonia-format-riff-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-format-riff/0.5.5) |
| symphonia-metadata-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-metadata/0.5.5) |
| symphonia-utils-xiph-0.5.5 | MPL-2.0 | [crates.io](https://crates.io/crates/symphonia-utils-xiph/0.5.5) |
| syn-1.0.109 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/syn/1.0.109) |
| syn-2.0.119 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/syn/2.0.119) |
| syn-3.0.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/syn/3.0.6) |
| sync_wrapper-1.0.2 | Apache-2.0 | [crates.io](https://crates.io/crates/sync_wrapper/1.0.2) |
| synstructure-0.14.0 | MIT | [crates.io](https://crates.io/crates/synstructure/0.14.0) |
| sysinfo-0.31.4 | MIT | [crates.io](https://crates.io/crates/sysinfo/0.31.4) |
| taffy-0.9.0 | MIT | [crates.io](https://crates.io/crates/taffy/0.9.0) |
| take-until-0.2.0 | MIT | [crates.io](https://crates.io/crates/take-until/0.2.0) |
| tempfile-3.27.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/tempfile/3.27.0) |
| tendril-0.4.3 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/tendril/0.4.3) |
| termcolor-1.4.1 | Unlicense OR MIT | [crates.io](https://crates.io/crates/termcolor/1.4.1) |
| thin-vec-0.2.20 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/thin-vec/0.2.20) |
| thiserror-1.0.69 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/thiserror/1.0.69) |
| thiserror-2.0.20 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/thiserror/2.0.20) |
| thiserror-impl-1.0.69 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/thiserror-impl/1.0.69) |
| thiserror-impl-2.0.20 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/thiserror-impl/2.0.20) |
| tiff-0.11.3 | MIT | [crates.io](https://crates.io/crates/tiff/0.11.3) |
| time-0.3.55 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/time/0.3.55) |
| time-core-0.1.9 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/time-core/0.1.9) |
| time-macros-0.2.32 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/time-macros/0.2.32) |
| tiny-keccak-2.0.2 | CC0-1.0 | [crates.io](https://crates.io/crates/tiny-keccak/2.0.2) |
| tiny-skia-0.11.4 | BSD-3-Clause | [crates.io](https://crates.io/crates/tiny-skia/0.11.4) |
| tiny-skia-path-0.11.4 | BSD-3-Clause | [crates.io](https://crates.io/crates/tiny-skia-path/0.11.4) |
| tinystr-0.8.4 | Unicode-3.0 | [crates.io](https://crates.io/crates/tinystr/0.8.4) |
| tinyvec-1.13.3 | Zlib OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/tinyvec/1.13.3) |
| tokio-1.53.1 | MIT | [crates.io](https://crates.io/crates/tokio/1.53.1) |
| tokio-rustls-0.26.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/tokio-rustls/0.26.5) |
| tokio-socks-0.5.3 | MIT | [crates.io](https://crates.io/crates/tokio-socks/0.5.3) |
| tokio-util-0.7.19 | MIT | [crates.io](https://crates.io/crates/tokio-util/0.7.19) |
| toml-0.8.23 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml/0.8.23) |
| toml-0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml/0.9.12+spec-1.1.0) |
| toml-1.1.6+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml/1.1.6+spec-1.1.0) |
| toml_datetime-0.6.11 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_datetime/0.6.11) |
| toml_datetime-0.7.5+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_datetime/0.7.5+spec-1.1.0) |
| toml_datetime-1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_datetime/1.1.1+spec-1.1.0) |
| toml_edit-0.22.27 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_edit/0.22.27) |
| toml_parser-1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_parser/1.1.3+spec-1.1.0) |
| toml_write-0.1.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_write/0.1.2) |
| toml_writer-1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/toml_writer/1.1.2+spec-1.1.0) |
| tower-0.5.3 | MIT | [crates.io](https://crates.io/crates/tower/0.5.3) |
| tower-http-0.6.11 | MIT | [crates.io](https://crates.io/crates/tower-http/0.6.11) |
| tower-layer-0.3.3 | MIT | [crates.io](https://crates.io/crates/tower-layer/0.3.3) |
| tower-service-0.3.3 | MIT | [crates.io](https://crates.io/crates/tower-service/0.3.3) |
| tracing-0.1.44 | MIT | [crates.io](https://crates.io/crates/tracing/0.1.44) |
| tracing-attributes-0.1.31 | MIT | [crates.io](https://crates.io/crates/tracing-attributes/0.1.31) |
| tracing-core-0.1.36 | MIT | [crates.io](https://crates.io/crates/tracing-core/0.1.36) |
| transpose-0.2.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/transpose/0.2.3) |
| tree-sitter-0.25.10 | MIT | [crates.io](https://crates.io/crates/tree-sitter/0.25.10) |
| tree-sitter-json-0.24.8 | MIT | [crates.io](https://crates.io/crates/tree-sitter-json/0.24.8) |
| tree-sitter-language-0.1.8 | MIT | [crates.io](https://crates.io/crates/tree-sitter-language/0.1.8) |
| triomphe-0.1.16 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/triomphe/0.1.16) |
| try-lock-0.2.5 | MIT | [crates.io](https://crates.io/crates/try-lock/0.2.5) |
| ttf-parser-0.25.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ttf-parser/0.25.1) |
| typeid-1.0.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/typeid/1.0.3) |
| typenum-1.20.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/typenum/1.20.1) |
| unicase-2.9.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicase/2.9.0) |
| unicode-bidi-0.3.18 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicode-bidi/0.3.18) |
| unicode-bidi-mirroring-0.4.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/unicode-bidi-mirroring/0.4.0) |
| unicode-ccc-0.4.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/unicode-ccc/0.4.0) |
| unicode-id-0.3.7 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicode-id/0.3.7) |
| unicode-ident-1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | [crates.io](https://crates.io/crates/unicode-ident/1.0.26) |
| unicode-properties-0.1.4 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/unicode-properties/0.1.4) |
| unicode-script-0.5.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicode-script/0.5.8) |
| unicode-segmentation-1.13.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicode-segmentation/1.13.3) |
| unicode-vo-0.1.0 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/unicode-vo/0.1.0) |
| unicode-width-0.2.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/unicode-width/0.2.2) |
| universal-hash-0.5.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/universal-hash/0.5.1) |
| unsafe-libyaml-0.2.11 | MIT | [crates.io](https://crates.io/crates/unsafe-libyaml/0.2.11) |
| untrusted-0.9.0 | ISC | [crates.io](https://crates.io/crates/untrusted/0.9.0) |
| ureq-3.4.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ureq/3.4.2) |
| ureq-proto-0.6.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/ureq-proto/0.6.4) |
| url-2.5.8 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/url/2.5.8) |
| usvg-0.45.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/usvg/0.45.1) |
| utf-8-0.7.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/utf-8/0.7.6) |
| utf8-zero-0.8.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/utf8-zero/0.8.1) |
| utf8_iter-1.0.4 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/utf8_iter/1.0.4) |
| utf8parse-0.2.2 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/utf8parse/0.2.2) |
| uuid-1.26.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/uuid/1.26.1) |
| v_frame-0.3.9 | BSD-2-Clause | [crates.io](https://crates.io/crates/v_frame/0.3.9) |
| value-bag-1.14.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/value-bag/1.14.1) |
| value-bag-serde1-1.14.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/value-bag-serde1/1.14.1) |
| value-bag-sval2-1.14.1 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/value-bag-sval2/1.14.1) |
| vcpkg-0.2.15 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/vcpkg/0.2.15) |
| version_check-0.9.5 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/version_check/0.9.5) |
| vswhom-0.1.0 | MIT | [crates.io](https://crates.io/crates/vswhom/0.1.0) |
| vswhom-sys-0.1.3 | MIT | [crates.io](https://crates.io/crates/vswhom-sys/0.1.3) |
| waker-fn-1.2.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/waker-fn/1.2.0) |
| walkdir-2.5.0 | Unlicense/MIT | [crates.io](https://crates.io/crates/walkdir/2.5.0) |
| want-0.3.1 | MIT | [crates.io](https://crates.io/crates/want/0.3.1) |
| wasapi-0.24.0 | MIT | [crates.io](https://crates.io/crates/wasapi/0.24.0) |
| wasm-bindgen-0.2.128 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/wasm-bindgen/0.2.128) |
| wasm-bindgen-macro-0.2.128 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/wasm-bindgen-macro/0.2.128) |
| wasm-bindgen-macro-support-0.2.128 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/wasm-bindgen-macro-support/0.2.128) |
| wasm-bindgen-shared-0.2.128 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/wasm-bindgen-shared/0.2.128) |
| webpki-root-certs-1.0.9 | CDLA-Permissive-2.0 | [crates.io](https://crates.io/crates/webpki-root-certs/1.0.9) |
| webpki-roots-1.0.9 | CDLA-Permissive-2.0 | [crates.io](https://crates.io/crates/webpki-roots/1.0.9) |
| weezl-0.1.12 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/weezl/0.1.12) |
| which-6.0.3 | MIT | [crates.io](https://crates.io/crates/which/6.0.3) |
| winapi-0.3.9 | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/winapi/0.3.9) |
| winapi-util-0.1.11 | Unlicense OR MIT | [crates.io](https://crates.io/crates/winapi-util/0.1.11) |
| windows-0.54.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows/0.54.0) |
| windows-0.57.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows/0.57.0) |
| windows-0.61.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows/0.61.3) |
| windows-0.62.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows/0.62.2) |
| windows-capture-1.5.0 | MIT | [crates.io](https://crates.io/crates/windows-capture/1.5.0) |
| windows-collections-0.2.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-collections/0.2.0) |
| windows-collections-0.3.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-collections/0.3.2) |
| windows-core-0.54.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-core/0.54.0) |
| windows-core-0.57.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-core/0.57.0) |
| windows-core-0.61.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-core/0.61.2) |
| windows-core-0.62.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-core/0.62.2) |
| windows-future-0.2.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-future/0.2.1) |
| windows-future-0.3.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-future/0.3.2) |
| windows-implement-0.57.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-implement/0.57.0) |
| windows-implement-0.60.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-implement/0.60.2) |
| windows-interface-0.57.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-interface/0.57.0) |
| windows-interface-0.59.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-interface/0.59.3) |
| windows-link-0.1.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-link/0.1.3) |
| windows-link-0.2.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-link/0.2.1) |
| windows-numerics-0.2.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-numerics/0.2.0) |
| windows-numerics-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-numerics/0.3.1) |
| windows-registry-0.4.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-registry/0.4.0) |
| windows-registry-0.5.3 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-registry/0.5.3) |
| windows-result-0.1.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-result/0.1.2) |
| windows-result-0.3.4 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-result/0.3.4) |
| windows-result-0.4.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-result/0.4.1) |
| windows-strings-0.3.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-strings/0.3.1) |
| windows-strings-0.4.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-strings/0.4.2) |
| windows-strings-0.5.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-strings/0.5.1) |
| windows-sys-0.52.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-sys/0.52.0) |
| windows-sys-0.59.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-sys/0.59.0) |
| windows-sys-0.60.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-sys/0.60.2) |
| windows-sys-0.61.2 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-sys/0.61.2) |
| windows-targets-0.52.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-targets/0.52.6) |
| windows-targets-0.53.5 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-targets/0.53.5) |
| windows-threading-0.1.0 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-threading/0.1.0) |
| windows-threading-0.2.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows-threading/0.2.1) |
| windows_x86_64_msvc-0.52.6 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows_x86_64_msvc/0.52.6) |
| windows_x86_64_msvc-0.53.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/windows_x86_64_msvc/0.53.1) |
| winnow-0.7.15 | MIT | [crates.io](https://crates.io/crates/winnow/0.7.15) |
| winnow-1.0.4 | MIT | [crates.io](https://crates.io/crates/winnow/1.0.4) |
| winreg-0.55.0 | MIT | [crates.io](https://crates.io/crates/winreg/0.55.0) |
| winsafe-0.0.19 | MIT | [crates.io](https://crates.io/crates/winsafe/0.0.19) |
| workspace-hack-0.1.0 | CC0-1.0 | [crates.io](https://crates.io/crates/workspace-hack/0.1.0) |
| writeable-0.6.4 | Unicode-3.0 | [crates.io](https://crates.io/crates/writeable/0.6.4) |
| xml5ever-0.18.1 | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/xml5ever/0.18.1) |
| xmlwriter-0.1.0 | MIT | [crates.io](https://crates.io/crates/xmlwriter/0.1.0) |
| y4m-0.8.0 | MIT | [crates.io](https://crates.io/crates/y4m/0.8.0) |
| yoke-0.8.3 | Unicode-3.0 | [crates.io](https://crates.io/crates/yoke/0.8.3) |
| yoke-derive-0.8.3 | Unicode-3.0 | [crates.io](https://crates.io/crates/yoke-derive/0.8.3) |
| zed-async-tar-0.5.0-zed | MIT/Apache-2.0 | [crates.io](https://crates.io/crates/zed-async-tar/0.5.0-zed) |
| zed-reqwest-0.12.15-zed | MIT OR Apache-2.0 | [crates.io](https://crates.io/crates/zed-reqwest/0.12.15-zed) |
| zed-scap-0.0.8-zed | MIT | [crates.io](https://crates.io/crates/zed-scap/0.0.8-zed) |
| zed-sum-tree-0.2.0 | Apache-2.0 | [crates.io](https://crates.io/crates/zed-sum-tree/0.2.0) |
| zerocopy-0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/zerocopy/0.8.57) |
| zerocopy-derive-0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/zerocopy-derive/0.8.57) |
| zerofrom-0.1.8 | Unicode-3.0 | [crates.io](https://crates.io/crates/zerofrom/0.1.8) |
| zerofrom-derive-0.1.8 | Unicode-3.0 | [crates.io](https://crates.io/crates/zerofrom-derive/0.1.8) |
| zeroize-1.9.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/zeroize/1.9.0) |
| zeroize_derive-1.5.0 | Apache-2.0 OR MIT | [crates.io](https://crates.io/crates/zeroize_derive/1.5.0) |
| zerotrie-0.2.5 | Unicode-3.0 | [crates.io](https://crates.io/crates/zerotrie/0.2.5) |
| zerovec-0.11.8 | Unicode-3.0 | [crates.io](https://crates.io/crates/zerovec/0.11.8) |
| zerovec-derive-0.11.6 | Unicode-3.0 | [crates.io](https://crates.io/crates/zerovec-derive/0.11.6) |
| zip-2.4.2 | MIT | [crates.io](https://crates.io/crates/zip/2.4.2) |
| zlib-rs-0.6.8 | Zlib | [crates.io](https://crates.io/crates/zlib-rs/0.6.8) |
| zmij-1.0.23 | MIT | [crates.io](https://crates.io/crates/zmij/1.0.23) |
| zopfli-0.8.3 | Apache-2.0 | [crates.io](https://crates.io/crates/zopfli/0.8.3) |
| zune-core-0.5.3 | MIT OR Apache-2.0 OR Zlib | [crates.io](https://crates.io/crates/zune-core/0.5.3) |
| zune-inflate-0.2.54 | MIT OR Apache-2.0 OR Zlib | [crates.io](https://crates.io/crates/zune-inflate/0.2.54) |
| zune-jpeg-0.5.15 | MIT OR Apache-2.0 OR Zlib | [crates.io](https://crates.io/crates/zune-jpeg/0.5.15) |

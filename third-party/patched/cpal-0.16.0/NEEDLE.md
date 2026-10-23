# cpal 0.16.0, patched for Needle

Android (AAudio) output and input also take 32-bit whole-number samples (`SampleFormat::I32`,
`AAUDIO_FORMAT_PCM_I32`, Android's `ENCODING_PCM_32BIT`), not only 16-bit and 32-bit float.
A bit-perfect USB DAC's mixer (Android 14) can ask for that format only, as the FiiO BTR17
does. The changes are marked "Needle" in src/host/aaudio.

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

SpectraForge — a Rust CLI (edition 2024) that turns an MP3 into an audio-reactive
video driven by a GLSL fragment shader. Pipeline: decode/analyze audio → render the
shader headlessly per frame → composite optional lyric/title overlays → mux with
ffmpeg into an MP4. See `README.md` for usage and shader authoring.

## Commands

```bash
cargo run            # build and run
cargo build          # build (use --release for optimized)
cargo test           # run all tests
cargo test <name>    # run a single test by name
cargo clippy         # lint
cargo fmt            # format
```

## Architecture

Single binary crate; `main.rs` exposes three subcommands — `transcribe`
(whisper → JSON), `align` (correct lyrics onto whisper timings), `render` (the
full video pipeline) — so each stage runs in isolation. Modules:

- `audio` — decode an MP3 to mono f32 samples (symphonia), or read duration only.
- `analysis` — `Analyzer` owns the decoded samples + a once-planned FFT; `.at(t)`
  returns per-frame `Features` (rms, bass/mid/treble, log-spaced spectrum).
  `Analyzer::silent()` is the duration-only variant.
- `render` — headless OpenGL `Renderer` (EGL/Mesa, or native CGL on macOS). Compiles
  the user shader into one or more passes and reads back rgb24 frames.
- `text` — `TextRenderer`: font loading + word-wrap layout + glyph rasterization
  (outline/shadow) onto rgb24 frames.
- `subtitle` — cue/timing model: parse SRT or Whisper JSON, hallucination filtering,
  per-word karaoke highlight timing, and `transcribe` (shells out to whisper).
- `align` — Needleman-Wunsch align ground-truth lyric text onto Whisper word
  timings (mishears keep timing, dropped words interpolate); emits corrected JSON.
- `lyrics` — the `Overlay` trait and its adapters: `LyricOverlay` (Plain/Mv styles,
  composes `TextRenderer` + cues) and `TitleOverlay` (fading title). `Overlays` owns
  the ordered stack and per-frame compositing.
- `encode` — `Encoder`: pipes rgb24 frames to an ffmpeg subprocess, muxing the
  original audio track.

Env knobs (set in `render`/`encode`): `SPECTRAFORGE_RENDER_BACKEND`,
`SPECTRAFORGE_EGL_LIBRARY`, `SPECTRAFORGE_VIDEO_CODEC`, `SPECTRAFORGE_VIDEO_BITRATE`.

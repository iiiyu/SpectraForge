# SpectraForge

Turn an MP3 into an audio-reactive video driven by a GLSL fragment shader.

Pipeline: `mp3 → decode → per-frame FFT analysis → GLSL uniforms → headless render → ffmpeg → mp4`

## Prerequisites

- **ffmpeg** on `PATH` (used for both decoding deps and final encoding).
- **Mesa EGL** for headless OpenGL: `libegl1`, `libgl1-mesa-dri`.
  On WSL2 this falls back to software (`llvmpipe`) — works, just slower.
- **whisper** on `PATH` (only for `--lyrics`): `pip install openai-whisper`.

## Usage

```bash
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 \
    [--width 1280] [--height 720] [--fps 30]

# Inspect audio features without rendering:
cargo run -- --input song.mp3 --shader vis.glsl --output x.mp4 --dump-features
```

### Lyrics as subtitles

Transcribe the song with whisper and burn the lyrics into the video:

```bash
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 \
    --lyrics [--whisper-model medium] [--whisper-cmd whisper]

# Or supply your own subtitle file (skips transcription):
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 \
    --subtitles song.srt
```

## Writing a shader

Shadertoy-style: define `mainImage`. These uniforms are injected automatically:

| Uniform | Type | Meaning |
|---------|------|---------|
| `iResolution` | `vec2` | output size in pixels |
| `iTime` | `float` | seconds |
| `iRMS` | `float` | overall loudness |
| `iBass` `iMid` `iTreble` | `float` | band energies (20–250 / 250–4k / 4k–20k Hz) |
| `iSpectrum` | `sampler2D` | 64×1 texture; sample `.r` for a log-spaced bin (0..1) |

See `vis.glsl` for a working example.

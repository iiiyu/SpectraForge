# SpectraForge

Turn an MP3 into an audio-reactive video driven by a GLSL fragment shader.

Pipeline: `mp3 → decode → per-frame FFT analysis → GLSL uniforms → headless render → ffmpeg → mp4`

## Prerequisites

- **ffmpeg** on `PATH` (used for both decoding deps and final encoding).
- **Mesa EGL** for headless OpenGL.
- **whisper** on `PATH` only when using `--lyrics`.

### macOS

Install the runtime tools with Homebrew:

```bash
brew install ffmpeg mesa
```

If `ffmpeg` is already installed, keep it and install only Mesa:

```bash
brew install mesa
```

Verify the commands and Mesa EGL library are visible:

```bash
ffmpeg -version
test -e "$(brew --prefix mesa)/lib/libEGL.dylib"
```

SpectraForge loads `libEGL` dynamically at runtime. If rendering fails with an
error such as `loading libEGL`, expose Homebrew's Mesa library directory before
running `cargo`:

```bash
export DYLD_FALLBACK_LIBRARY_PATH="$(brew --prefix mesa)/lib:${DYLD_FALLBACK_LIBRARY_PATH:-}"
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4
```

Or point SpectraForge directly at the EGL library:

```bash
export SPECTRAFORGE_EGL_LIBRARY="$(brew --prefix mesa)/lib/libEGL.dylib"
```

On macOS, SpectraForge uses native CGL/OpenGL by default, which should report an
Apple renderer such as `Apple M1 Max` and an OpenGL version containing `Metal` at
startup. This keeps GLSL shader compatibility while using Apple's hardware path.
Mesa EGL remains available as a fallback or for non-interactive environments:

```bash
SPECTRAFORGE_RENDER_BACKEND=metal cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4
SPECTRAFORGE_RENDER_BACKEND=egl cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4
```

This is a Metal-backed OpenGL path, not a direct Metal Shading Language backend.
A pure Metal renderer would require a separate shader format or a GLSL-to-MSL
translation pipeline.

macOS also defaults to FFmpeg's hardware H.264 encoder (`h264_videotoolbox`).
Override the encoder or bitrate if needed:

```bash
SPECTRAFORGE_VIDEO_CODEC=libx264 cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4
SPECTRAFORGE_VIDEO_BITRATE=24M cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4
```

To keep that setting for future zsh sessions:

```bash
echo 'export DYLD_FALLBACK_LIBRARY_PATH="$(brew --prefix mesa)/lib:${DYLD_FALLBACK_LIBRARY_PATH:-}"' >> ~/.zshrc
```

Whisper is optional and only needed when using `--lyrics`. Install it with `uv`
so the `whisper` command is available on `PATH`:

```bash
uv tool install openai-whisper
whisper --help
```

If `uv` warns that its tool executable directory is not on `PATH`, update your
shell configuration and open a new terminal:

```bash
uv tool update-shell
```

When you want lyrics, run SpectraForge from a shell where `whisper` is available:

```bash
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 --lyrics
```

### Linux / WSL2

```bash
sudo apt update
sudo apt install ffmpeg libegl1 libgl1-mesa-dri
uv tool install openai-whisper  # only for --lyrics
```

On WSL2, Mesa usually falls back to software rendering (`llvmpipe`). That works,
but is slower than hardware rendering.

## Usage

```bash
cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 \
    [--width 1280] [--height 720] [--fps 30]

cargo run --release -- --input song.mp3 --shader vis.glsl --output out.mp4 --width 1920 --height 1080 --fps 30

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

| Uniform                  | Type        | Meaning                                               |
| ------------------------ | ----------- | ----------------------------------------------------- |
| `iResolution`            | `vec2`      | output size in pixels                                 |
| `iTime`                  | `float`     | seconds                                               |
| `iRMS`                   | `float`     | overall loudness                                      |
| `iBass` `iMid` `iTreble` | `float`     | band energies (20–250 / 250–4k / 4k–20k Hz)           |
| `iSpectrum`              | `sampler2D` | 64×1 texture; sample `.r` for a log-spaced bin (0..1) |

See `vis.glsl` for a working example.

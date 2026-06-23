# SpectraForge

Turn an MP3 into an audio-reactive video driven by a GLSL fragment shader.

Pipeline: `mp3 → decode/duration → GLSL uniforms → headless render → styled lyrics/audio mux → mp4`

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
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4
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
SPECTRAFORGE_RENDER_BACKEND=metal cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4
SPECTRAFORGE_RENDER_BACKEND=egl cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4
```

This is a Metal-backed OpenGL path, not a direct Metal Shading Language backend.
A pure Metal renderer would require a separate shader format or a GLSL-to-MSL
translation pipeline.

macOS also defaults to FFmpeg's hardware H.264 encoder (`h264_videotoolbox`).
Override the encoder or bitrate if needed:

```bash
SPECTRAFORGE_VIDEO_CODEC=libx264 cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4
SPECTRAFORGE_VIDEO_BITRATE=24M cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4
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
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 --lyrics
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
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    [--width 1280] [--height 720] [--fps 30]

cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 --width 1920 --height 1080 --fps 30

# Use the MP3 only for duration/output audio; shader audio uniforms stay silent:
cargo run --release -- --input song.mp3 --shader shaders/without_audio/rover_seasons_loop.glsl --output out.mp4 --duration-only

# Inspect audio features without rendering:
cargo run -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output x.mp4 --dump-features
```

Rendered videos include the input MP3 audio track. `--duration-only` only turns
off audio-reactive shader features; it does not mute or remove the output audio.

### Lyrics as subtitles

Transcribe the song with whisper and burn the lyrics into the video:

```bash
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --lyrics [--whisper-model medium] [--whisper-cmd whisper]

# Or supply your own subtitle file (skips transcription):
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --subtitles song.srt
```

Lyrics default to SpectraForge's built-in animated MV/TikTok-style renderer with
a bold custom font, black outline/shadow, fade/slide/scale motion, and timed
word highlighting. The lyrics are composited into each RGB frame before ffmpeg
encodes the final MP4, so this does not depend on ffmpeg's `subtitles`,
`drawtext`, or libass filters.

`--lyrics` asks whisper for a `.json` transcript with `--word_timestamps True`
and uses those per-word times for karaoke highlighting, so the first word does
not highlight before its actual timestamp. `--subtitles` accepts either `.srt`
or whisper `.json`; SRT has only cue-level timing, so SpectraForge applies a
small first-word delay as a fallback. To reduce Whisper tail hallucinations,
cue-like phrases with more than 3 words but less than 0.5s of source timing are
dropped before rendering.

```bash
cargo run --release -- --input song.mp3 --shader shaders/without_audio/rover_seasons_loop.glsl --output out.mp4 \
    --lyrics \
    --subtitle-font "Arial Rounded MT Bold" \
    --subtitle-font-size 72

# Use fonts from a custom font directory:
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --subtitles song.json \
    --subtitle-font "My Display Font" \
    --subtitle-fonts-dir ./fonts

# Keep the old plain subtitle renderer:
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --lyrics --subtitle-style plain
```

### Title

A centered title is shown by default for the first 3s and then fades out. With
no flags it uses the MP3 file stem (e.g. `song.mp3` → `song`); the filename is
just a convenience default, not a promise it's the real song name.

```bash
# Override the text:
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --title "My Song"

# Turn the title off:
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --no-title

# Style the title independently of the lyrics (each defaults to its --subtitle-* counterpart):
cargo run --release -- --input song.mp3 --shader shaders/with_audio/vis.glsl --output out.mp4 \
    --title "My Song" \
    --title-font "Georgia" --title-font-size 96 --title-fonts-dir ./fonts \
    --title-duration 4.5
```

`--title-duration` sets how many seconds the title stays fully visible before
fading out (default `3`); the fade-in/out timing is fixed. If the resolved title
is empty (an input with no usable file stem), no title is drawn. `--title-font`,
`--title-font-size`, and `--title-fonts-dir` each fall back to the corresponding
`--subtitle-*` value when unset.

When a requested font can't be found, SpectraForge falls back to any installed
font rather than failing, and prints a warning naming the substitute.

## Writing a shader

Shadertoy-style: define `mainImage`. These uniforms are injected automatically:

| Uniform                  | Type        | Meaning                                               |
| ------------------------ | ----------- | ----------------------------------------------------- |
| `iResolution`            | `vec2`      | output size in pixels                                 |
| `iTime`                  | `float`     | seconds                                               |
| `iRMS`                   | `float`     | overall loudness                                      |
| `iBass` `iMid` `iTreble` | `float`     | band energies (20–250 / 250–4k / 4k–20k Hz)           |
| `iSpectrum`              | `sampler2D` | 64×1 texture; sample `.r` for a log-spaced bin (0..1) |

Example shaders live under `shaders/without_audio/` — they animate on `iTime`
alone and render identically for any input, so pair them with `--duration-only`
(the MP3 is still muxed as the sound track). See
`shaders/without_audio/limacon_glow.glsl` for a working example.

### Multipass shaders

A shader file can declare several passes, separated by a line that starts with
`//---pass`. Each pass renders to its own texture; later passes sample earlier
ones via `iPass1`, `iPass2`, … (pass 1 is `iPass1`, and so on). The last pass is
the image written to video. A file with no `//---pass` marker is a single pass,
exactly as before.

```glsl
// Pass 1 -> iPass1
void mainImage(out vec4 c, in vec2 p) { c = vec4(p / iResolution.xy, 0.0, 1.0); }
//---pass---
// Pass 2 (final): composite the earlier pass
void mainImage(out vec4 c, in vec2 p) { c = texture(iPass1, p / iResolution.xy); }
```

See `shaders/without_audio/feedback_bloom.glsl` for a 3-pass example.

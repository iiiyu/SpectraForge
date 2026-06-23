# SpectraForge

A CLI that turns an MP3 into an audio-reactive video driven by a GLSL shader,
with optional lyric and title overlays muxed into an MP4.

## Language

### Overlays

**Title**:
Intro text shown centered for the first few seconds, then faded out. Defaults
to the MP3 file stem as a convenience — it is *not* a promise that the filename
is the song's real name. The user owns it via `--title "..."` (override) or
`--no-title` (suppress). Garbage filename in = garbage title out is acceptable.
An empty resolved title means no title overlay. The Title is independently
styleable (its own font / size / fonts-dir), falling back to the corresponding
Subtitle styling when unset.

**Title hold**:
How long the Title stays fully visible before fading out (`--title-duration`,
default 3s). Fade-in (400ms) and fade-out (700ms) are fixed product polish, not
user knobs.
_Avoid_: Heading, caption

**Overlay**:
Anything that draws itself onto a rendered rgb24 frame for a given timestamp
(the `Overlay` trait). Composited in order over each frame.

**Lyric overlay**:
An overlay that burns timed subtitle cues into frames (Plain or Mv style).
_Avoid_: Subtitle (reserve "subtitle" for the cue/timing source data)

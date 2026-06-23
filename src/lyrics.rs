use crate::subtitle::{SubtitleCue, highlighted_word, parse_subtitles};
use crate::text::{TextRenderer, ease_out, effective_font_size};
use anyhow::{Context, Result, ensure};
use std::path::Path;

/// Something that draws itself onto an rgb24 frame for a given timestamp.
pub trait Overlay {
    fn draw(&self, frame: &mut [u8], time_seconds: f32);
}

/// An ordered stack of overlays composited onto each rendered frame. Owns the
/// scratch buffer and the no-overlay fast path so callers just hand it a frame.
pub struct Overlays {
    overlays: Vec<Box<dyn Overlay>>,
    scratch: Vec<u8>,
}

impl Overlays {
    pub fn new(overlays: Vec<Box<dyn Overlay>>) -> Self {
        Self {
            overlays,
            scratch: Vec::new(),
        }
    }

    /// Composite all overlays (in order) over `frame` at time `t`, returning the
    /// result. With no overlays, returns `frame` untouched (no copy).
    pub fn composite<'a>(&'a mut self, frame: &'a [u8], t: f32) -> &'a [u8] {
        if self.overlays.is_empty() {
            return frame;
        }
        self.scratch.clear();
        self.scratch.extend_from_slice(frame);
        for overlay in &self.overlays {
            overlay.draw(&mut self.scratch, t);
        }
        &self.scratch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayStyle {
    Plain,
    Mv,
}

const TITLE_FADE_IN_MS: u64 = 400;
const TITLE_FADE_OUT_MS: u64 = 700;

/// Burns subtitle cues into rgb24 frames. Holds a `TextRenderer` for
/// rasterization and the parsed cues + timing model; `draw` selects the active
/// cue for a timestamp and styles it.
pub struct LyricOverlay {
    cues: Vec<SubtitleCue>,
    renderer: TextRenderer,
    font_size: u32,
    style: OverlayStyle,
}

impl LyricOverlay {
    pub fn from_subtitles(
        subtitles: &Path,
        width: u32,
        height: u32,
        font_name: &str,
        requested_font_size: u32,
        fonts_dir: Option<&Path>,
        style: OverlayStyle,
    ) -> Result<Self> {
        ensure!(
            !subtitles
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ass")),
            "ASS input is not supported by the built-in lyric renderer; use an SRT or Whisper JSON file"
        );

        let contents = std::fs::read_to_string(subtitles)
            .with_context(|| format!("reading subtitles {}", subtitles.display()))?;
        let cues = parse_subtitles(subtitles, &contents)
            .with_context(|| format!("parsing subtitles {}", subtitles.display()))?;
        let font_size = effective_font_size(height, requested_font_size);
        let renderer = TextRenderer::new(width, height, font_name, fonts_dir)?;
        let timed_cues = cues.iter().filter(|cue| !cue.words.is_empty()).count();
        eprintln!(
            "styled lyrics with built-in renderer: {} cues, {} with word timings, font \"{}\" @ {}px",
            cues.len(),
            timed_cues,
            font_name,
            font_size
        );

        Ok(Self {
            cues,
            renderer,
            font_size,
            style,
        })
    }

    fn draw_plain(&self, frame: &mut [u8], cue: &SubtitleCue) {
        let lines = self.renderer.layout(&cue.text, self.font_size, 2);
        let scale = self.font_size as f32;
        let alpha = 1.0;
        let target_y = self.renderer.height() as f32 * 0.82;
        self.renderer
            .draw_lines(frame, &lines, scale, target_y, alpha, None);
    }

    fn draw_mv(&self, frame: &mut [u8], cue: &SubtitleCue, time_ms: u64) {
        let local_ms = time_ms.saturating_sub(cue.start_ms);
        let remaining_ms = cue.end_ms.saturating_sub(time_ms);
        let fade_in = ease_out((local_ms as f32 / 180.0).clamp(0.0, 1.0));
        let fade_out = ease_out((remaining_ms as f32 / 160.0).clamp(0.0, 1.0));
        let intro = ease_out((local_ms as f32 / 220.0).clamp(0.0, 1.0));
        let alpha = fade_in * fade_out;
        let scale = self.font_size as f32 * (0.92 + intro * 0.1);
        let height = self.renderer.height() as f32;
        let target_y = height * 0.78 + (1.0 - intro) * height * 0.025;

        let highlighted = highlighted_word(cue, time_ms);
        let lines = self.renderer.layout(&cue.text, scale.round() as u32, 2);
        self.renderer
            .draw_lines(frame, &lines, scale, target_y, alpha, highlighted);
    }
}

impl Overlay for LyricOverlay {
    fn draw(&self, frame: &mut [u8], time_seconds: f32) {
        let time_ms = (time_seconds.max(0.0) * 1000.0).round() as u64;
        let Some(cue) = self
            .cues
            .iter()
            .rev()
            .find(|cue| cue.start_ms <= time_ms && time_ms < cue.end_ms)
        else {
            return;
        };

        match self.style {
            OverlayStyle::Plain => self.draw_plain(frame, cue),
            OverlayStyle::Mv => self.draw_mv(frame, cue, time_ms),
        }
    }
}

/// A centered title shown for the first few seconds, then faded out. Owns its
/// own fade timing — it is not a subtitle cue.
pub struct TitleOverlay {
    renderer: TextRenderer,
    text: String,
    font_size: u32,
    hold_ms: u64,
}

impl TitleOverlay {
    pub fn new(
        text: &str,
        width: u32,
        height: u32,
        font_name: &str,
        requested_font_size: u32,
        fonts_dir: Option<&Path>,
        hold_seconds: f32,
    ) -> Result<Self> {
        let font_size = effective_font_size(height, requested_font_size);
        let renderer = TextRenderer::new(width, height, font_name, fonts_dir)?;
        let hold_ms = (hold_seconds.max(0.0) * 1000.0).round() as u64;
        Ok(Self {
            renderer,
            text: text.to_string(),
            font_size,
            hold_ms,
        })
    }
}

impl Overlay for TitleOverlay {
    fn draw(&self, frame: &mut [u8], time_seconds: f32) {
        let time_ms = (time_seconds.max(0.0) * 1000.0).round() as u64;
        if time_ms >= self.hold_ms + TITLE_FADE_OUT_MS {
            return;
        }
        let alpha = if time_ms < TITLE_FADE_IN_MS {
            time_ms as f32 / TITLE_FADE_IN_MS as f32
        } else if time_ms < self.hold_ms {
            1.0
        } else {
            1.0 - (time_ms - self.hold_ms) as f32 / TITLE_FADE_OUT_MS as f32
        };
        let scale = self.font_size as f32 * 1.25;
        let lines = self.renderer.layout(&self.text, scale.round() as u32, 2);
        let center_y = self.renderer.height() as f32 * 0.5;
        self.renderer
            .draw_lines(frame, &lines, scale, center_y, alpha, None);
    }
}

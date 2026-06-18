use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use anyhow::{Context, Result, bail, ensure};
use fontdb::{Database, Family, Query, Weight};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Transcribe `audio` to an SRT subtitle file using the `whisper` CLI.
///
/// Runs e.g. `whisper song.mp3 --model medium --output_format srt
/// --output_dir <dir>`, which writes `<stem>.srt` into `out_dir`. Returns the
/// path to that file, or `None` if whisper produced no subtitle cues (e.g. an
/// instrumental track with no detectable speech).
pub fn transcribe(
    audio: &Path,
    whisper_cmd: &str,
    model: &str,
    out_dir: &Path,
) -> Result<Option<PathBuf>> {
    eprintln!("transcribing lyrics with {whisper_cmd} (model {model})...");
    let status = Command::new(whisper_cmd)
        .arg(audio)
        .args(["--model", model])
        .args(["--output_format", "srt"])
        .arg("--output_dir")
        .arg(out_dir)
        .status()
        .with_context(|| format!("running {whisper_cmd} (is it installed and on PATH?)"))?;
    if !status.success() {
        bail!("{whisper_cmd} exited with status {status}");
    }

    let stem = audio.file_stem().context("input audio has no file name")?;
    let srt = out_dir.join(Path::new(stem)).with_extension("srt");
    if !srt.exists() {
        bail!("expected subtitle file not found: {}", srt.display());
    }

    // An empty SRT (no cues) breaks ffmpeg's subtitles filter; treat it as
    // "no lyrics" instead.
    let contents =
        std::fs::read_to_string(&srt).with_context(|| format!("reading {}", srt.display()))?;
    if contents.trim().is_empty() {
        eprintln!("no lyrics detected; skipping subtitles");
        return Ok(None);
    }
    Ok(Some(srt))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubtitleCue {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayStyle {
    Plain,
    Mv,
}

pub struct LyricOverlay {
    cues: Vec<SubtitleCue>,
    font: FontVec,
    width: u32,
    height: u32,
    font_size: u32,
    style: OverlayStyle,
}

#[derive(Clone, Copy)]
struct TextPlacement {
    x: f32,
    baseline: f32,
}

#[derive(Clone, Copy)]
struct TextStyle {
    scale: f32,
    color: [u8; 3],
    alpha: f32,
}

impl LyricOverlay {
    pub fn from_srt(
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
            "ASS input is not supported by the built-in lyric renderer; use an SRT file"
        );

        let contents = std::fs::read_to_string(subtitles)
            .with_context(|| format!("reading subtitles {}", subtitles.display()))?;
        let cues = parse_srt(&contents)
            .with_context(|| format!("parsing SRT subtitles {}", subtitles.display()))?;
        let font_size = effective_font_size(height, requested_font_size);
        let font = load_font(font_name, fonts_dir)?;
        eprintln!(
            "styled lyrics with built-in renderer: {} cues, font \"{}\" @ {}px",
            cues.len(),
            font_name,
            font_size
        );

        Ok(Self {
            cues,
            font,
            width,
            height,
            font_size,
            style,
        })
    }

    pub fn draw(&self, frame: &mut [u8], time_seconds: f32) {
        let time_ms = (time_seconds.max(0.0) * 1000.0).round() as u64;
        let Some(cue) = self
            .cues
            .iter()
            .find(|cue| cue.start_ms <= time_ms && time_ms < cue.end_ms)
        else {
            return;
        };

        match self.style {
            OverlayStyle::Plain => self.draw_plain(frame, cue),
            OverlayStyle::Mv => self.draw_mv(frame, cue, time_ms),
        }
    }

    fn draw_plain(&self, frame: &mut [u8], cue: &SubtitleCue) {
        let lines = layout_words(&cue.text, self.width, self.font_size, 2);
        let scale = self.font_size as f32;
        let alpha = 1.0;
        let target_y = self.height as f32 * 0.82;
        self.draw_lines(frame, &lines, scale, target_y, alpha, None);
    }

    fn draw_mv(&self, frame: &mut [u8], cue: &SubtitleCue, time_ms: u64) {
        let duration_ms = (cue.end_ms - cue.start_ms).max(1);
        let local_ms = time_ms.saturating_sub(cue.start_ms);
        let remaining_ms = cue.end_ms.saturating_sub(time_ms);
        let progress = (local_ms as f32 / duration_ms as f32).clamp(0.0, 1.0);
        let fade_in = ease_out((local_ms as f32 / 180.0).clamp(0.0, 1.0));
        let fade_out = ease_out((remaining_ms as f32 / 160.0).clamp(0.0, 1.0));
        let intro = ease_out((local_ms as f32 / 220.0).clamp(0.0, 1.0));
        let alpha = fade_in * fade_out;
        let scale = self.font_size as f32 * (0.92 + intro * 0.1);
        let target_y = self.height as f32 * 0.78 + (1.0 - intro) * self.height as f32 * 0.025;

        let words = cue.text.split_whitespace().count().max(1);
        let highlighted = ((progress * words as f32).floor() as usize).min(words - 1);
        let lines = layout_words(&cue.text, self.width, scale.round() as u32, 2);
        self.draw_lines(frame, &lines, scale, target_y, alpha, Some(highlighted));
    }

    fn draw_lines(
        &self,
        frame: &mut [u8],
        lines: &[Vec<(usize, String)>],
        scale: f32,
        target_y: f32,
        alpha: f32,
        highlighted: Option<usize>,
    ) {
        if lines.is_empty() || alpha <= 0.0 {
            return;
        }

        let scaled = self.font.as_scaled(PxScale::from(scale));
        let line_height = (scaled.height() * 1.12).max(scale);
        let block_height = line_height * lines.len() as f32;
        let top = target_y - block_height * 0.5;
        let space_width = self.measure_text(" ", scale);

        for (line_idx, line) in lines.iter().enumerate() {
            let line_width = self.measure_line(line, scale, space_width);
            let mut x = (self.width as f32 - line_width) * 0.5;
            let baseline = top + scaled.ascent() + line_idx as f32 * line_height;

            for (word_idx, word) in line {
                let fill = match highlighted {
                    Some(last) if *word_idx <= last => [255, 216, 64],
                    Some(_) => [255, 255, 255],
                    None => [255, 255, 255],
                };
                self.draw_word(
                    frame,
                    word,
                    TextPlacement { x, baseline },
                    TextStyle {
                        scale,
                        color: fill,
                        alpha,
                    },
                );
                x += self.measure_text(word, scale) + space_width;
            }
        }
    }

    fn draw_word(&self, frame: &mut [u8], word: &str, placement: TextPlacement, style: TextStyle) {
        let outline = ((style.scale * 0.08).round() as i32).clamp(2, 6);
        let shadow = ((style.scale * 0.07).round() as i32).clamp(2, 5);

        self.draw_text_run(
            frame,
            word,
            TextPlacement {
                x: placement.x + shadow as f32,
                baseline: placement.baseline + shadow as f32,
            },
            TextStyle {
                scale: style.scale,
                color: [0, 0, 0],
                alpha: style.alpha * 0.42,
            },
        );

        for dy in -outline..=outline {
            for dx in -outline..=outline {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if dx * dx + dy * dy <= outline * outline {
                    self.draw_text_run(
                        frame,
                        word,
                        TextPlacement {
                            x: placement.x + dx as f32,
                            baseline: placement.baseline + dy as f32,
                        },
                        TextStyle {
                            scale: style.scale,
                            color: [0, 0, 0],
                            alpha: style.alpha * 0.85,
                        },
                    );
                }
            }
        }

        self.draw_text_run(frame, word, placement, style);
    }

    fn draw_text_run(
        &self,
        frame: &mut [u8],
        text: &str,
        placement: TextPlacement,
        style: TextStyle,
    ) {
        let scaled = self.font.as_scaled(PxScale::from(style.scale));
        let mut caret = placement.x;
        let mut previous = None;

        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }

            let glyph = id.with_scale_and_position(
                PxScale::from(style.scale),
                point(caret, placement.baseline),
            );
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|glyph_x, glyph_y, coverage| {
                    let px = bounds.min.x as i32 + glyph_x as i32;
                    let py = bounds.min.y as i32 + glyph_y as i32;
                    self.blend_pixel(frame, px, py, style.color, coverage * style.alpha);
                });
            }

            caret += scaled.h_advance(id);
            previous = Some(id);
        }
    }

    fn measure_line(&self, line: &[(usize, String)], scale: f32, space_width: f32) -> f32 {
        line.iter()
            .enumerate()
            .map(|(idx, (_, word))| {
                self.measure_text(word, scale) + if idx > 0 { space_width } else { 0.0 }
            })
            .sum()
    }

    fn measure_text(&self, text: &str, scale: f32) -> f32 {
        let scaled = self.font.as_scaled(PxScale::from(scale));
        let mut width = 0.0;
        let mut previous = None;

        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(previous) = previous {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            previous = Some(id);
        }

        width
    }

    fn blend_pixel(&self, frame: &mut [u8], x: i32, y: i32, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }

        let idx = ((y as u32 * self.width + x as u32) * 3) as usize;
        for channel in 0..3 {
            let src = color[channel] as f32;
            let dst = frame[idx + channel] as f32;
            frame[idx + channel] = (dst * (1.0 - alpha) + src * alpha).round() as u8;
        }
    }
}

fn load_font(font_name: &str, fonts_dir: Option<&Path>) -> Result<FontVec> {
    let mut db = Database::new();
    if let Some(path) = fonts_dir {
        if path.is_file() {
            db.load_font_file(path)
                .with_context(|| format!("loading font file {}", path.display()))?;
        } else {
            db.load_fonts_dir(path);
        }
    }
    db.load_system_fonts();

    let named = [Family::Name(font_name), Family::SansSerif];
    let fallback = [Family::SansSerif];
    let id = db
        .query(&Query {
            families: &named,
            weight: Weight::BOLD,
            ..Query::default()
        })
        .or_else(|| {
            db.query(&Query {
                families: &fallback,
                weight: Weight::BOLD,
                ..Query::default()
            })
        })
        .with_context(|| format!("could not find subtitle font \"{font_name}\""))?;

    let (data, face_index) = db
        .with_face_data(id, |data, face_index| (data.to_vec(), face_index))
        .context("loading subtitle font bytes")?;
    FontVec::try_from_vec_and_index(data, face_index).context("parsing subtitle font")
}

fn layout_words(
    text: &str,
    width: u32,
    font_size: u32,
    max_lines: usize,
) -> Vec<Vec<(usize, String)>> {
    let max_chars = max_chars_per_line(width, font_size);
    let mut lines: Vec<Vec<(usize, String)>> = vec![Vec::new()];
    let mut line_chars = 0usize;

    for (idx, word) in text.split_whitespace().enumerate() {
        let word = escape_draw_text(word);
        if word.is_empty() {
            continue;
        }

        let word_len = word.chars().count();
        let add_len = if line_chars == 0 {
            word_len
        } else {
            word_len + 1
        };
        if line_chars > 0 && line_chars + add_len > max_chars && lines.len() < max_lines {
            lines.push(Vec::new());
            line_chars = 0;
        }

        if line_chars > 0 {
            line_chars += 1;
        }
        line_chars += word_len;
        lines
            .last_mut()
            .expect("layout always has a line")
            .push((idx, word));
    }

    lines.into_iter().filter(|line| !line.is_empty()).collect()
}

fn escape_draw_text(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r' | '\t'))
        .collect()
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn parse_srt(contents: &str) -> Result<Vec<SubtitleCue>> {
    let normalized = contents.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();

    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }

        let timing_idx = lines
            .iter()
            .position(|line| line.contains("-->"))
            .with_context(|| format!("SRT block is missing a timing line: {}", lines[0]))?;
        let (start_ms, end_ms) = parse_timing_line(lines[timing_idx])?;
        if end_ms <= start_ms {
            continue;
        }

        let text = clean_srt_text(&lines[timing_idx + 1..].join(" "));
        if text.is_empty() {
            continue;
        }

        cues.push(SubtitleCue {
            start_ms,
            end_ms,
            text,
        });
    }

    ensure!(!cues.is_empty(), "subtitle file contains no cues");
    Ok(cues)
}

fn parse_timing_line(line: &str) -> Result<(u64, u64)> {
    let (start, end) = line
        .split_once("-->")
        .with_context(|| format!("invalid SRT timing line: {line}"))?;
    let end = end
        .split_whitespace()
        .next()
        .context("SRT timing line has no end time")?;
    Ok((parse_srt_time(start)?, parse_srt_time(end)?))
}

fn parse_srt_time(raw: &str) -> Result<u64> {
    let raw = raw.trim();
    let (hms, frac) = raw
        .split_once(',')
        .or_else(|| raw.split_once('.'))
        .with_context(|| format!("invalid SRT timestamp: {raw}"))?;
    let parts: Vec<&str> = hms.split(':').collect();
    ensure!(parts.len() == 3, "invalid SRT timestamp: {raw}");

    let hours: u64 = parts[0].parse()?;
    let minutes: u64 = parts[1].parse()?;
    let seconds: u64 = parts[2].parse()?;
    let mut millis = frac
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .take(3)
        .collect::<String>();
    while millis.len() < 3 {
        millis.push('0');
    }
    let millis: u64 = millis.parse()?;

    Ok((((hours * 60 + minutes) * 60 + seconds) * 1000) + millis)
}

fn clean_srt_text(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn max_chars_per_line(width: u32, font_size: u32) -> usize {
    (width as f32 / (font_size as f32 * 0.52))
        .floor()
        .clamp(16.0, 42.0) as usize
}

fn effective_font_size(height: u32, requested: u32) -> u32 {
    if requested > 0 {
        requested
    } else {
        ((height as f32 * 0.085).round() as u32).clamp(34, 92)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_blocks() {
        let cues = parse_srt(
            "1\n00:00:00,000 --> 00:00:02,500\nHello <i>wide</i> world\n\n\
             2\n00:00:02,500 --> 00:00:04,000\nNext line\n",
        )
        .unwrap();

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 0);
        assert_eq!(cues[0].end_ms, 2500);
        assert_eq!(cues[0].text, "Hello wide world");
    }

    #[test]
    fn layout_wraps_words_to_two_lines() {
        let lines = layout_words(
            "Driving through the night with luggage on the roof",
            320,
            32,
            2,
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0][0].1, "Driving");
    }
}

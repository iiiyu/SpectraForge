use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use anyhow::{Context, Result};
use fontdb::{Database, Family, Query, Weight};
use std::path::Path;

/// Glyph rasterizer: loads a bold font once, then lays out and draws word-wrapped
/// lines of text into an rgb24 frame with a black outline + drop shadow. Owns the
/// frame geometry so callers pass only text, scale, position, and alpha.
pub struct TextRenderer {
    font: FontVec,
    width: u32,
    height: u32,
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

impl TextRenderer {
    pub fn new(width: u32, height: u32, font_name: &str, fonts_dir: Option<&Path>) -> Result<Self> {
        let font = load_font(font_name, fonts_dir)?;
        Ok(Self {
            font,
            width,
            height,
        })
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Word-wrap `text` to at most `max_lines` lines, keyed by word index so a
    /// caller can highlight up to a given word. Width is derived from geometry.
    pub fn layout(
        &self,
        text: &str,
        font_size: u32,
        max_lines: usize,
    ) -> Vec<Vec<(usize, String)>> {
        layout_words(text, self.width, font_size, max_lines)
    }

    /// Draw pre-laid-out `lines`, centered horizontally, with their vertical
    /// block centered on `target_y`. Words with index `<= highlighted` are drawn
    /// in the karaoke fill colour.
    pub fn draw_lines(
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

/// Cubic ease-out, for fade/slide/scale animation curves.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Resolve the subtitle font size: honour an explicit request, else derive from
/// the frame height.
pub fn effective_font_size(height: u32, requested: u32) -> u32 {
    if requested > 0 {
        requested
    } else {
        ((height as f32 * 0.085).round() as u32).clamp(34, 92)
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

fn max_chars_per_line(width: u32, font_size: u32) -> usize {
    (width as f32 / (font_size as f32 * 0.52))
        .floor()
        .clamp(16.0, 42.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

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

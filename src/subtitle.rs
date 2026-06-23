use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const WORD_HIGHLIGHT_LEAD_MS: u64 = 20;
const WORD_DISPLAY_LEAD_MS: u64 = 360;
const WORD_DISPLAY_TRAIL_MS: u64 = 260;
const SHORT_HALLUCINATION_MAX_MS: u64 = 500;

/// One subtitle cue: a span of text shown over `[start_ms, end_ms)`, optionally
/// with per-word timings for karaoke highlighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub words: Vec<TimedWord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Transcribe `audio` to a Whisper JSON file with word timestamps.
///
/// Runs e.g. `whisper song.mp3 --model medium --word_timestamps True
/// --fp16 False --output_format json --output_dir <dir>`, which writes `<stem>.json` into
/// `out_dir`. Returns the path to that file, or `None` if whisper produced no
/// lyric cues (e.g. an instrumental track with no detectable speech).
pub fn transcribe(
    audio: &Path,
    whisper_cmd: &str,
    model: &str,
    out_dir: &Path,
) -> Result<Option<PathBuf>> {
    eprintln!("transcribing lyrics with {whisper_cmd} (model {model}, word timestamps)...");
    let status = Command::new(whisper_cmd)
        .arg(audio)
        .args(["--model", model])
        .args(["--word_timestamps", "True"])
        // FP16 produces all-NaN logits on some GPUs (e.g. NVIDIA T1200), making
        // whisper crash mid-decode. FP32 is slightly slower but stable.
        .args(["--fp16", "False"])
        .args(["--output_format", "json"])
        .arg("--output_dir")
        .arg(out_dir)
        .status()
        .with_context(|| format!("running {whisper_cmd} (is it installed and on PATH?)"))?;
    if !status.success() {
        bail!("{whisper_cmd} exited with status {status}");
    }

    let stem = audio.file_stem().context("input audio has no file name")?;
    let json = out_dir.join(Path::new(stem)).with_extension("json");
    if !json.exists() {
        bail!("expected whisper JSON file not found: {}", json.display());
    }

    let contents =
        std::fs::read_to_string(&json).with_context(|| format!("reading {}", json.display()))?;
    if parse_whisper_json(&contents)?.is_empty() {
        eprintln!("no lyrics detected; skipping subtitles");
        return Ok(None);
    }
    Ok(Some(json))
}

/// Index of the latest word that should be highlighted at `time_ms`, or `None`
/// before the first word. Uses real per-word timings when present, otherwise a
/// proportional estimate for coarse cue-level subtitles.
pub fn highlighted_word(cue: &SubtitleCue, time_ms: u64) -> Option<usize> {
    if !cue.words.is_empty() {
        return cue
            .words
            .iter()
            .enumerate()
            .take_while(|(_, word)| word.start_ms <= time_ms.saturating_add(WORD_HIGHLIGHT_LEAD_MS))
            .map(|(idx, _)| idx)
            .last();
    }

    let word_count = cue.text.split_whitespace().count();
    if word_count == 0 {
        return None;
    }

    let duration_ms = cue.end_ms.saturating_sub(cue.start_ms).max(1);
    let local_ms = time_ms.saturating_sub(cue.start_ms);
    let first_word_delay = estimated_first_word_delay_ms(duration_ms, word_count);
    if local_ms < first_word_delay {
        return None;
    }

    let active_ms = duration_ms
        .saturating_sub(first_word_delay)
        .max(word_count as u64 * 120);
    let progress = ((local_ms - first_word_delay) as f32 / active_ms as f32).clamp(0.0, 1.0);
    Some(((progress * word_count as f32).floor() as usize).min(word_count - 1))
}

fn estimated_first_word_delay_ms(duration_ms: u64, word_count: usize) -> u64 {
    let proportional = duration_ms / 12;
    let half_word_slot = duration_ms / ((word_count as u64 + 1) * 2);
    proportional.max(half_word_slot).clamp(180, 1200)
}

fn is_short_hallucinated_phrase(text: &str, start_ms: u64, end_ms: u64) -> bool {
    text.split_whitespace().count() > 3
        && end_ms.saturating_sub(start_ms) < SHORT_HALLUCINATION_MAX_MS
}

/// Parse an SRT or Whisper-JSON subtitle file (chosen by extension) into cues
/// sorted by start time.
pub fn parse_subtitles(path: &Path, contents: &str) -> Result<Vec<SubtitleCue>> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let mut cues = if ext.eq_ignore_ascii_case("json") {
        parse_whisper_json(contents)?
    } else {
        parse_srt(contents)?
    };

    cues.sort_by_key(|cue| cue.start_ms);
    ensure!(!cues.is_empty(), "subtitle file contains no cues");
    Ok(cues)
}

#[derive(Debug, Deserialize)]
struct WhisperOutput {
    #[serde(default)]
    segments: Vec<WhisperSegment>,
}

#[derive(Debug, Deserialize)]
struct WhisperSegment {
    start: Option<f64>,
    end: Option<f64>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<WhisperWord>,
}

#[derive(Debug, Deserialize)]
struct WhisperWord {
    word: String,
    start: Option<f64>,
    end: Option<f64>,
}

fn parse_whisper_json(contents: &str) -> Result<Vec<SubtitleCue>> {
    let output: WhisperOutput = serde_json::from_str(contents).context("parsing Whisper JSON")?;
    let mut cues = Vec::new();

    for segment in output.segments {
        let mut words = segment
            .words
            .into_iter()
            .filter_map(|word| {
                let text = clean_srt_text(&word.word);
                if text.is_empty() {
                    return None;
                }
                let start_ms = seconds_to_ms(word.start?)?;
                let end_ms = seconds_to_ms(word.end?)?;
                if end_ms <= start_ms {
                    return None;
                }
                Some(TimedWord {
                    start_ms,
                    end_ms,
                    text,
                })
            })
            .collect::<Vec<_>>();
        words.sort_by_key(|word| word.start_ms);

        let text = if words.is_empty() {
            clean_srt_text(&segment.text)
        } else {
            words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };
        if text.is_empty() {
            continue;
        }

        let segment_start_ms = segment
            .start
            .and_then(seconds_to_ms)
            .or_else(|| words.first().map(|word| word.start_ms))
            .unwrap_or(0);
        let segment_end_ms = segment
            .end
            .and_then(seconds_to_ms)
            .or_else(|| words.last().map(|word| word.end_ms))
            .unwrap_or(segment_start_ms.saturating_add(1000));
        if is_short_hallucinated_phrase(&text, segment_start_ms, segment_end_ms) {
            continue;
        }

        let start_ms = words
            .first()
            .map(|word| word.start_ms.saturating_sub(WORD_DISPLAY_LEAD_MS))
            .unwrap_or(segment_start_ms);
        let end_ms = words
            .last()
            .map(|word| word.end_ms.saturating_add(WORD_DISPLAY_TRAIL_MS))
            .unwrap_or(segment_end_ms)
            .max(segment_end_ms);
        if end_ms <= start_ms {
            continue;
        }

        cues.push(SubtitleCue {
            start_ms,
            end_ms,
            text,
            words,
        });
    }

    Ok(cues)
}

fn seconds_to_ms(seconds: f64) -> Option<u64> {
    if seconds.is_finite() && seconds >= 0.0 {
        Some((seconds * 1000.0).round() as u64)
    } else {
        None
    }
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
        if is_short_hallucinated_phrase(&text, start_ms, end_ms) {
            continue;
        }

        cues.push(SubtitleCue {
            start_ms,
            end_ms,
            text,
            words: Vec::new(),
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
        assert!(cues[0].words.is_empty());
    }

    #[test]
    fn parses_whisper_json_word_timestamps() {
        let cues = parse_whisper_json(
            r#"{
                "segments": [{
                    "start": 0.0,
                    "end": 3.0,
                    "text": " Hello world",
                    "words": [
                        {"word": " Hello", "start": 1.2, "end": 1.5},
                        {"word": " world", "start": 2.0, "end": 2.3}
                    ]
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start_ms, 840);
        assert_eq!(cues[0].end_ms, 3000);
        assert_eq!(cues[0].text, "Hello world");
        assert_eq!(cues[0].words[0].start_ms, 1200);
    }

    #[test]
    fn filters_short_whisper_hallucinated_phrases() {
        let cues = parse_whisper_json(
            r#"{
                "segments": [
                    {
                        "start": 201.52,
                        "end": 201.88,
                        "text": " Bad boy, bad boy, that's that old",
                        "words": []
                    },
                    {
                        "start": 204.0,
                        "end": 205.2,
                        "text": " Bad boy, bad boy",
                        "words": []
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Bad boy, bad boy");
    }

    #[test]
    fn word_timestamps_wait_for_first_word() {
        let cue = parse_whisper_json(
            r#"{
                "segments": [{
                    "start": 0.0,
                    "end": 3.0,
                    "text": " Hello world",
                    "words": [
                        {"word": " Hello", "start": 1.2, "end": 1.5},
                        {"word": " world", "start": 2.0, "end": 2.3}
                    ]
                }]
            }"#,
        )
        .unwrap()
        .remove(0);

        assert_eq!(highlighted_word(&cue, 1179), None);
        assert_eq!(highlighted_word(&cue, 1200), Some(0));
        assert_eq!(highlighted_word(&cue, 2000), Some(1));
    }

    #[test]
    fn coarse_srt_does_not_highlight_immediately() {
        let cue = parse_srt("1\n00:00:00,000 --> 00:00:04,000\nHello world\n")
            .unwrap()
            .remove(0);

        assert_eq!(highlighted_word(&cue, 0), None);
        assert_eq!(highlighted_word(&cue, 500), None);
        assert_eq!(highlighted_word(&cue, 700), Some(0));
    }

    #[test]
    fn filters_short_srt_hallucinated_phrases() {
        let cues = parse_srt(
            "1\n00:03:21,520 --> 00:03:21,880\nBad boy, bad boy, that's that old\n\n\
             2\n00:03:24,000 --> 00:03:25,200\nBad boy, bad boy\n",
        )
        .unwrap();

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Bad boy, bad boy");
    }
}

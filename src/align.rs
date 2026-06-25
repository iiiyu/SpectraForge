//! Align ground-truth lyrics onto Whisper word timings.
//!
//! Whisper hears the *timing* well but mishears *words*. Given the correct
//! lyrics as plain text plus a Whisper JSON, we sequence-align the two word
//! streams (Needleman-Wunsch) and transfer each Whisper word's timing onto the
//! corresponding correct word, interpolating where Whisper dropped a word.
//! Output is a Whisper-shaped JSON (one segment per lyric line) that the
//! renderer consumes unchanged.

use crate::subtitle::{TimedWord, parse_subtitles};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::path::Path;

const DEFAULT_WORD_MS: u64 = 300;

#[derive(Serialize)]
struct OutDoc {
    text: String,
    segments: Vec<OutSegment>,
}

#[derive(Serialize)]
struct OutSegment {
    start: f64,
    end: f64,
    text: String,
    words: Vec<OutWord>,
}

#[derive(Serialize)]
struct OutWord {
    word: String,
    start: f64,
    end: f64,
}

/// Read `lyrics_path` (plain text) and `whisper_path` (Whisper JSON), align
/// them, and return a corrected Whisper-shaped JSON string.
pub fn align_files(lyrics_path: &Path, whisper_path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(lyrics_path)
        .with_context(|| format!("reading lyrics {}", lyrics_path.display()))?;
    let whisper = std::fs::read_to_string(whisper_path)
        .with_context(|| format!("reading whisper json {}", whisper_path.display()))?;
    let cues = parse_subtitles(whisper_path, &whisper)
        .with_context(|| format!("parsing whisper json {}", whisper_path.display()))?;
    let whisper_words: Vec<TimedWord> = cues.into_iter().flat_map(|c| c.words).collect();
    ensure!(
        !whisper_words.is_empty(),
        "whisper json has no word timings to align against (need --word_timestamps output)"
    );

    let doc = align(&raw, &whisper_words);
    serde_json::to_string_pretty(&doc).context("serializing aligned json")
}

/// Split raw lyric text into lines of words, dropping blank lines and
/// `[Section]` headers.
fn lyric_lines(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !(l.starts_with('[') && l.ends_with(']')))
        .map(|l| l.split_whitespace().map(str::to_string).collect())
        .filter(|words: &Vec<String>| !words.is_empty())
        .collect()
}

/// Lowercase, keep only ascii alphanumerics — for matching only, never display.
fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn align(raw: &str, whisper: &[TimedWord]) -> OutDoc {
    let lines = lyric_lines(raw);
    let raw_words: Vec<String> = lines.iter().flatten().cloned().collect();

    let a: Vec<String> = raw_words.iter().map(|w| normalize(w)).collect();
    let b: Vec<String> = whisper.iter().map(|w| normalize(&w.text)).collect();
    let matched = align_indices(&a, &b);

    // Pull timings through matched positions; leave gaps as None to interpolate.
    let mut starts: Vec<Option<u64>> = vec![None; raw_words.len()];
    let mut ends: Vec<Option<u64>> = vec![None; raw_words.len()];
    for (i, m) in matched.iter().enumerate() {
        if let Some(j) = m {
            starts[i] = Some(whisper[*j].start_ms);
            ends[i] = Some(whisper[*j].end_ms);
        }
    }
    interpolate(&mut starts, &mut ends);

    // Build one block per lyric line (text + timed words).
    let mut blocks: Vec<OutSegment> = Vec::new();
    let mut idx = 0;
    for line in &lines {
        let mut words = Vec::new();
        for word in line {
            words.push(OutWord {
                word: format!(" {word}"),
                start: starts[idx].unwrap_or(0) as f64 / 1000.0,
                end: ends[idx].unwrap_or(0) as f64 / 1000.0,
            });
            idx += 1;
        }
        blocks.push(OutSegment {
            start: words.first().map(|w| w.start).unwrap_or(0.0),
            end: words.last().map(|w| w.end).unwrap_or(0.0),
            text: line.join(" "),
            words,
        });
    }

    OutDoc {
        text: raw_words.join(" "),
        segments: merge_for_readability(blocks),
    }
}

/// A single lyric line often flashes by faster than it can be read. Merge
/// consecutive lines into one cue until it stays on screen at least
/// `MIN_DISPLAY_MS`, capped at `MAX_LINES_PER_CUE` lines, and never bridging a
/// gap longer than `MAX_GAP_MS` (an instrumental break should still clear the
/// screen).
fn merge_for_readability(blocks: Vec<OutSegment>) -> Vec<OutSegment> {
    const MIN_DISPLAY_MS: f64 = 2800.0;
    const MAX_GAP_MS: f64 = 2500.0;
    const MAX_LINES_PER_CUE: usize = 2;

    let mut out: Vec<OutSegment> = Vec::new();
    let mut lines_in_cur = 0usize;
    for block in blocks {
        let display_ms = out
            .last()
            .map(|c| (c.end - c.start) * 1000.0)
            .unwrap_or(f64::INFINITY);
        let gap_ms = out
            .last()
            .map(|c| (block.start - c.end) * 1000.0)
            .unwrap_or(0.0);

        match out.last_mut() {
            Some(cur)
                if display_ms < MIN_DISPLAY_MS
                    && lines_in_cur < MAX_LINES_PER_CUE
                    && gap_ms <= MAX_GAP_MS =>
            {
                cur.end = block.end;
                cur.text.push(' ');
                cur.text.push_str(&block.text);
                cur.words.extend(block.words);
                lines_in_cur += 1;
            }
            _ => {
                out.push(block);
                lines_in_cur = 1;
            }
        }
    }
    out
}

/// Needleman-Wunsch. Returns, for each token in `a`, the diagonally-aligned
/// index in `b` (or None for a gap). Substitutions still align diagonally so a
/// misheard word inherits its Whisper timing.
fn align_indices(a: &[String], b: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (a.len(), b.len());
    let mut score = vec![vec![0i32; m + 1]; n + 1];
    // ponytail: indexing by row/col is the clearest way to write the NW borders.
    #[allow(clippy::needless_range_loop)]
    for i in 0..=n {
        score[i][0] = -(i as i32);
    }
    #[allow(clippy::needless_range_loop)]
    for j in 0..=m {
        score[0][j] = -(j as i32);
    }
    for i in 1..=n {
        for j in 1..=m {
            let s = if a[i - 1] == b[j - 1] { 1 } else { -1 };
            score[i][j] = (score[i - 1][j - 1] + s)
                .max(score[i - 1][j] - 1)
                .max(score[i][j - 1] - 1);
        }
    }

    let mut res = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let s = if a[i - 1] == b[j - 1] { 1 } else { -1 };
        if score[i][j] == score[i - 1][j - 1] + s {
            res[i - 1] = Some(j - 1);
            i -= 1;
            j -= 1;
        } else if score[i][j] == score[i - 1][j] - 1 {
            i -= 1; // raw word Whisper missed → gap, interpolate later
        } else {
            j -= 1; // Whisper word with no raw counterpart → drop it
        }
    }
    res
}

/// Fill `None` timings: runs between two anchors split the gap evenly; leading
/// / trailing runs fall back to a fixed per-word slot.
fn interpolate(starts: &mut [Option<u64>], ends: &mut [Option<u64>]) {
    let n = starts.len();
    let mut i = 0;
    while i < n {
        if starts[i].is_some() {
            i += 1;
            continue;
        }
        let run_start = i;
        while i < n && starts[i].is_none() {
            i += 1;
        }
        let run_len = (i - run_start) as u64;
        let prev_end = run_start.checked_sub(1).and_then(|p| ends[p]);
        let next_start = starts.get(i).copied().flatten();

        match (prev_end, next_start) {
            (Some(pe), Some(ns)) if ns > pe => {
                let slot = (ns - pe) / run_len;
                for (k, idx) in (run_start..i).enumerate() {
                    let s = pe + slot * k as u64;
                    starts[idx] = Some(s);
                    ends[idx] = Some(s + slot.max(1));
                }
            }
            (Some(pe), _) => {
                for (k, idx) in (run_start..i).enumerate() {
                    let s = pe + DEFAULT_WORD_MS * k as u64;
                    starts[idx] = Some(s);
                    ends[idx] = Some(s + DEFAULT_WORD_MS);
                }
            }
            (None, Some(ns)) => {
                for (k, idx) in (run_start..i).enumerate() {
                    let from_end = (run_len - k as u64) * DEFAULT_WORD_MS;
                    let s = ns.saturating_sub(from_end);
                    starts[idx] = Some(s);
                    ends[idx] = Some(s + DEFAULT_WORD_MS);
                }
            }
            (None, None) => {
                for (k, idx) in (run_start..i).enumerate() {
                    let s = DEFAULT_WORD_MS * k as u64;
                    starts[idx] = Some(s);
                    ends[idx] = Some(s + DEFAULT_WORD_MS);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start_ms: u64, end_ms: u64) -> TimedWord {
        TimedWord {
            start_ms,
            end_ms,
            text: text.to_string(),
        }
    }

    #[test]
    fn transfers_timing_through_mishears_and_interpolates_gaps() {
        // Raw line: "big car big swing"; Whisper misheard "car"->"cart" and
        // dropped "big" (the 3rd word) entirely.
        let whisper = vec![
            w("big", 1000, 1200),
            w("cart", 1200, 1500),
            w("swing", 2000, 2300),
        ];
        let doc = align("[Verse]\nbig car big swing\n", &whisper);

        assert_eq!(doc.segments.len(), 1);
        let words = &doc.segments[0].words;
        assert_eq!(
            words.iter().map(|x| x.word.trim()).collect::<Vec<_>>(),
            ["big", "car", "big", "swing"]
        );
        // First and last anchor to real Whisper timings; everything in between
        // is monotonic and bounded by them (mishear inherits a timing, the
        // dropped word is interpolated — whichever "big" the tie resolves to).
        assert_eq!(words[0].start, 1.0);
        assert_eq!(words[3].start, 2.0);
        for pair in words.windows(2) {
            assert!(pair[0].start <= pair[1].start, "timings must not go backwards");
        }
        for word in &words[1..3] {
            assert!(word.start >= 1.0 && word.end <= 2.0, "interior word out of range");
        }
    }

    fn seg(text: &str, start: f64, end: f64) -> OutSegment {
        OutSegment {
            start,
            end,
            text: text.to_string(),
            words: vec![OutWord {
                word: format!(" {text}"),
                start,
                end,
            }],
        }
    }

    #[test]
    fn merge_pairs_short_lines_but_caps_at_two_and_breaks_on_gaps() {
        let merged = merge_for_readability(vec![
            seg("line one", 0.0, 1.0),   // short → merges with next
            seg("line two", 1.0, 2.0),   // pairs with line one (2-line cap)
            seg("line three", 2.0, 3.0), // cap hit → starts a new cue
            seg("after break", 9.0, 11.0), // 6s gap → never merges backward
        ]);

        let texts: Vec<&str> = merged.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["line one line two", "line three", "after break"]);
        assert_eq!(merged[0].end, 2.0); // span covers both merged lines
        assert_eq!(merged[0].words.len(), 2);
    }
}

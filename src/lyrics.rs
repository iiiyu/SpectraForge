use anyhow::{Context, Result, bail};
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

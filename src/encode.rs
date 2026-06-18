use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Wraps an ffmpeg subprocess fed raw rgb24 frames on stdin, muxing in the
/// original audio track from `audio_path`.
pub struct Encoder {
    child: Child,
}

impl Encoder {
    pub fn new(
        output: &Path,
        audio_path: &Path,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self> {
        let child = Command::new("ffmpeg")
            .args(["-y", "-f", "rawvideo", "-pix_fmt", "rgb24"])
            .args(["-s", &format!("{width}x{height}")])
            .args(["-r", &fps.to_string()])
            .args(["-i", "-"]) // video frames from stdin
            .arg("-i")
            .arg(audio_path) // audio from original file
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-c:a", "aac", "-shortest"])
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawning ffmpeg (is it installed and on PATH?)")?;
        Ok(Self { child })
    }

    /// Write one rgb24 frame (`width * height * 3` bytes).
    pub fn write_frame(&mut self, rgb: &[u8]) -> Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("ffmpeg stdin already closed")?;
        stdin.write_all(rgb).context("writing frame to ffmpeg")?;
        Ok(())
    }

    /// Close stdin and wait for ffmpeg to finish encoding.
    pub fn finish(mut self) -> Result<()> {
        drop(self.child.stdin.take()); // EOF on stdin
        let status = self.child.wait().context("waiting for ffmpeg")?;
        if !status.success() {
            bail!("ffmpeg exited with status {status}");
        }
        Ok(())
    }
}

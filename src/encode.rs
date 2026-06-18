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
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24"])
            .args(["-s", &format!("{width}x{height}")])
            .args(["-r", &fps.to_string()])
            .args(["-i", "-"]) // video frames from stdin
            .arg("-i")
            .arg(audio_path); // audio from original file

        // Be explicit: raw frames provide the video stream, input audio provides the sound.
        cmd.args(["-map", "0:v:0", "-map", "1:a:0"]);

        let video_codec = std::env::var("SPECTRAFORGE_VIDEO_CODEC")
            .unwrap_or_else(|_| default_video_codec().to_string());
        cmd.args(["-c:v", &video_codec, "-pix_fmt", "yuv420p"]);
        if video_codec == "h264_videotoolbox" || video_codec == "hevc_videotoolbox" {
            let bitrate = std::env::var("SPECTRAFORGE_VIDEO_BITRATE")
                .unwrap_or_else(|_| default_video_bitrate(width, height));
            cmd.args(["-b:v", &bitrate, "-allow_sw", "1"]);
        }

        let child = cmd
            .args(["-c:a", "aac", "-shortest"])
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
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
        let output = self
            .child
            .wait_with_output()
            .context("waiting for ffmpeg")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "ffmpeg exited with status {}: {}",
                output.status,
                stderr.trim()
            );
        }
        Ok(())
    }
}

fn default_video_codec() -> &'static str {
    if cfg!(target_os = "macos") {
        "h264_videotoolbox"
    } else {
        "libx264"
    }
}

fn default_video_bitrate(width: u32, height: u32) -> String {
    let pixels = width as u64 * height as u64;
    let mbps = if pixels <= 1280 * 720 {
        8
    } else if pixels <= 1920 * 1080 {
        16
    } else {
        32
    };
    format!("{mbps}M")
}

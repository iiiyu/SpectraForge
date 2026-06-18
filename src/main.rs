mod analysis;
mod audio;
mod encode;
mod lyrics;
mod render;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Turn an MP3 into an audio-reactive video driven by a GLSL fragment shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum SubtitleStyle {
    /// Plain built-in centered subtitle rendering.
    Plain,
    /// Animated MV/TikTok-style lyrics with built-in styling.
    Mv,
}

#[derive(Parser, Debug)]
#[command(name = "spectraforge", version, about)]
struct Args {
    /// Input MP3 file
    #[arg(short, long)]
    input: PathBuf,

    /// Fragment shader (.glsl), Shadertoy-style `mainImage`
    #[arg(short, long)]
    shader: PathBuf,

    /// Output video file
    #[arg(short, long)]
    output: PathBuf,

    #[arg(long, default_value_t = 1280)]
    width: u32,

    #[arg(long, default_value_t = 720)]
    height: u32,

    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Transcribe lyrics with whisper and render them into the video
    #[arg(long)]
    lyrics: bool,

    /// Whisper command to invoke for transcription
    #[arg(long, default_value = "whisper")]
    whisper_cmd: String,

    /// Whisper model to use (e.g. tiny, base, small, medium, large)
    #[arg(long, default_value = "medium")]
    whisper_model: String,

    /// Use an existing .srt subtitle file or Whisper .json instead of transcribing
    #[arg(long)]
    subtitles: Option<PathBuf>,

    /// Subtitle rendering style
    #[arg(long, value_enum, default_value = "mv")]
    subtitle_style: SubtitleStyle,

    /// Font family for MV-style lyrics
    #[arg(long, default_value = "Arial Rounded MT Bold")]
    subtitle_font: String,

    /// Font size for MV-style lyrics; 0 chooses a size from video height
    #[arg(long, default_value_t = 0)]
    subtitle_font_size: u32,

    /// Directory containing custom font files for the subtitle renderer
    #[arg(long)]
    subtitle_fonts_dir: Option<PathBuf>,

    /// Use input audio only for output duration/audio; do not drive shader features from it
    #[arg(long)]
    duration_only: bool,

    /// Print audio features per frame and exit (no rendering)
    #[arg(long)]
    dump_features: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (duration, audio_data) = if args.duration_only {
        let duration = audio::duration(&args.input).context("reading input audio duration")?;
        (duration, None)
    } else {
        let (samples, sample_rate) = audio::decode(&args.input).context("decoding input audio")?;
        let duration = samples.len() as f32 / sample_rate as f32;
        (duration, Some((samples, sample_rate)))
    };
    let total_frames = (duration * args.fps as f32).ceil() as usize;
    if let Some((_, sample_rate)) = &audio_data {
        eprintln!(
            "decoded {:.1}s @ {} Hz -> {} frames @ {} fps",
            duration, sample_rate, total_frames, args.fps
        );
    } else {
        eprintln!(
            "duration-only {:.1}s -> {} frames @ {} fps",
            duration, total_frames, args.fps
        );
    }

    if args.dump_features {
        let silent_features = analysis::silent();
        for i in 0..total_frames {
            let t = i as f32 / args.fps as f32;
            let features = audio_data
                .as_ref()
                .map(|(samples, sample_rate)| analysis::analyze(samples, *sample_rate, t));
            let f = features.as_ref().unwrap_or(&silent_features);
            println!(
                "{:6.3}s rms={:.4} bass={:.4} mid={:.4} treble={:.4}",
                t, f.rms, f.bass, f.mid, f.treble
            );
        }
        return Ok(());
    }

    // Resolve subtitles: an explicit subtitle file wins; otherwise transcribe if asked.
    let subtitles = match (&args.subtitles, args.lyrics) {
        (Some(srt), _) => Some(srt.clone()),
        (None, true) => {
            let out_dir = args.output.parent().filter(|p| !p.as_os_str().is_empty());
            let out_dir = out_dir.unwrap_or_else(|| std::path::Path::new("."));
            lyrics::transcribe(&args.input, &args.whisper_cmd, &args.whisper_model, out_dir)
                .context("transcribing lyrics")?
        }
        (None, false) => None,
    };
    let lyric_overlay = match subtitles {
        Some(path) => {
            let style = match args.subtitle_style {
                SubtitleStyle::Plain => lyrics::OverlayStyle::Plain,
                SubtitleStyle::Mv => lyrics::OverlayStyle::Mv,
            };
            Some(
                lyrics::LyricOverlay::from_subtitles(
                    &path,
                    args.width,
                    args.height,
                    &args.subtitle_font,
                    args.subtitle_font_size,
                    args.subtitle_fonts_dir.as_deref(),
                    style,
                )
                .context("preparing lyric overlay")?,
            )
        }
        None => None,
    };

    let shader_src = std::fs::read_to_string(&args.shader)
        .with_context(|| format!("reading shader {}", args.shader.display()))?;

    let mut renderer = render::Renderer::new(args.width, args.height, &shader_src)
        .context("initializing renderer")?;
    let mut encoder =
        encode::Encoder::new(&args.output, &args.input, args.width, args.height, args.fps)
            .context("starting ffmpeg encoder")?;

    let silent_features = analysis::silent();
    let mut composited_frame = Vec::new();
    for i in 0..total_frames {
        let t = i as f32 / args.fps as f32;
        let frame = if let Some((samples, sample_rate)) = &audio_data {
            let features = analysis::analyze(samples, *sample_rate, t);
            renderer.render_frame(t, &features)
        } else {
            renderer.render_frame(t, &silent_features)
        };
        if let Some(lyric_overlay) = &lyric_overlay {
            composited_frame.clear();
            composited_frame.extend_from_slice(frame);
            lyric_overlay.draw(&mut composited_frame, t);
            encoder.write_frame(&composited_frame)?;
        } else {
            encoder.write_frame(frame)?;
        }
        if i % args.fps as usize == 0 {
            eprint!("\rrendering {}/{}", i, total_frames);
        }
    }
    eprintln!("\rrendered {} frames        ", total_frames);

    encoder.finish().context("finalizing video")?;
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

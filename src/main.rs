mod align;
mod analysis;
mod audio;
mod encode;
mod lyrics;
mod render;
mod subtitle;
mod text;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Video aspect ratio presets. Each maps to a 1080p YouTube-ready resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Aspect {
    /// 16:9 landscape, 1920x1080 — standard YouTube videos.
    #[value(name = "16:9")]
    Wide,
    /// 9:16 portrait, 1080x1920 — YouTube Shorts / TikTok / Reels.
    #[value(name = "9:16")]
    Tall,
}

impl Aspect {
    fn dimensions(self) -> (u32, u32) {
        match self {
            Aspect::Wide => (1920, 1080),
            Aspect::Tall => (1080, 1920),
        }
    }
}

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
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Transcribe an MP3 to a Whisper JSON with word timings (step 1).
    Transcribe(TranscribeArgs),
    /// Align ground-truth lyrics onto Whisper timings → corrected JSON (step 2).
    Align(AlignArgs),
    /// Render the audio-reactive video, optionally with overlays (step 3).
    Render(RenderArgs),
}

#[derive(Parser, Debug)]
struct TranscribeArgs {
    /// Input MP3 file
    #[arg(short, long)]
    input: PathBuf,

    /// Output JSON path (default: <input stem>.json next to the input)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Whisper command to invoke for transcription
    #[arg(long, default_value = "whisper")]
    whisper_cmd: String,

    /// Whisper model to use (e.g. tiny, base, small, medium, large)
    #[arg(long, default_value = "medium")]
    whisper_model: String,
}

#[derive(Parser, Debug)]
struct AlignArgs {
    /// Plain-text ground-truth lyrics (one line per lyric line)
    #[arg(short, long)]
    lyrics: PathBuf,

    /// Whisper JSON with word timings (from `transcribe`)
    #[arg(short, long)]
    whisper: PathBuf,

    /// Output corrected JSON path
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Parser, Debug)]
struct RenderArgs {
    /// Input MP3 file
    #[arg(short, long)]
    input: PathBuf,

    /// Fragment shader (.glsl), Shadertoy-style `mainImage`
    #[arg(short, long)]
    shader: PathBuf,

    /// Output video file
    #[arg(short, long)]
    output: PathBuf,

    /// Video aspect: `16:9` (YouTube, 1920x1080) or `9:16` (Shorts, 1080x1920).
    #[arg(long, value_enum, default_value = "16:9")]
    aspect: Aspect,

    /// Override width in pixels (otherwise derived from --aspect).
    #[arg(long)]
    width: Option<u32>,

    /// Override height in pixels (otherwise derived from --aspect).
    #[arg(long)]
    height: Option<u32>,

    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// SRT or Whisper/aligned JSON file to render as lyrics (from `transcribe`/`align`)
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

    /// Title text shown for the first 3s, then fades out. Defaults to the MP3 file name.
    #[arg(long)]
    title: Option<String>,

    /// Suppress the title overlay (otherwise it shows the MP3 file name).
    #[arg(long)]
    no_title: bool,

    /// Font family for the title. Defaults to --subtitle-font.
    #[arg(long)]
    title_font: Option<String>,

    /// Font size for the title; 0 chooses a size from video height. Defaults to --subtitle-font-size.
    #[arg(long)]
    title_font_size: Option<u32>,

    /// Directory of custom font files for the title. Defaults to --subtitle-fonts-dir.
    #[arg(long)]
    title_fonts_dir: Option<PathBuf>,

    /// Seconds the title stays fully visible before fading out (fades are fixed).
    #[arg(long, default_value_t = 3.0)]
    title_duration: f32,

    /// Use input audio only for output duration/audio; do not drive shader features from it
    #[arg(long)]
    duration_only: bool,

    /// Print audio features per frame and exit (no rendering)
    #[arg(long)]
    dump_features: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Transcribe(args) => transcribe(args),
        Command::Align(args) => {
            let json = align::align_files(&args.lyrics, &args.whisper)?;
            std::fs::write(&args.output, json)
                .with_context(|| format!("writing {}", args.output.display()))?;
            eprintln!("wrote {}", args.output.display());
            Ok(())
        }
        Command::Render(args) => render(args),
    }
}

fn transcribe(args: TranscribeArgs) -> Result<()> {
    let out_dir = args
        .output
        .as_ref()
        .and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .or_else(|| args.input.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let json = subtitle::transcribe(
        &args.input,
        &args.whisper_cmd,
        &args.whisper_model,
        &out_dir,
    )
    .context("transcribing lyrics")?
    .context("no lyrics detected in audio")?;
    // Whisper writes <stem>.json into out_dir; rename if the user picked a path.
    if let Some(output) = &args.output
        && output != &json
    {
        std::fs::rename(&json, output)
            .with_context(|| format!("moving {} -> {}", json.display(), output.display()))?;
        eprintln!("wrote {}", output.display());
    } else {
        eprintln!("wrote {}", json.display());
    }
    Ok(())
}

fn render(args: RenderArgs) -> Result<()> {
    // --width/--height override the --aspect preset independently.
    let (aw, ah) = args.aspect.dimensions();
    let width = args.width.unwrap_or(aw);
    let height = args.height.unwrap_or(ah);

    let (duration, analyzer) = if args.duration_only {
        let duration = audio::duration(&args.input).context("reading input audio duration")?;
        let total_frames = (duration * args.fps as f32).ceil() as usize;
        eprintln!(
            "duration-only {:.1}s -> {} frames @ {} fps",
            duration, total_frames, args.fps
        );
        (duration, analysis::Analyzer::silent())
    } else {
        let (samples, sample_rate) = audio::decode(&args.input).context("decoding input audio")?;
        let duration = samples.len() as f32 / sample_rate as f32;
        let total_frames = (duration * args.fps as f32).ceil() as usize;
        eprintln!(
            "decoded {:.1}s @ {} Hz -> {} frames @ {} fps",
            duration, sample_rate, total_frames, args.fps
        );
        (duration, analysis::Analyzer::reactive(samples, sample_rate))
    };
    let total_frames = (duration * args.fps as f32).ceil() as usize;

    if args.dump_features {
        for i in 0..total_frames {
            let t = i as f32 / args.fps as f32;
            let f = analyzer.at(t);
            println!(
                "{:6.3}s rms={:.4} bass={:.4} mid={:.4} treble={:.4}",
                t, f.rms, f.bass, f.mid, f.treble
            );
        }
        return Ok(());
    }

    // Overlays composite in order: lyrics first (lower), title on top.
    let mut overlay_stack: Vec<Box<dyn lyrics::Overlay>> = Vec::new();
    if let Some(path) = args.subtitles {
        let style = match args.subtitle_style {
            SubtitleStyle::Plain => lyrics::OverlayStyle::Plain,
            SubtitleStyle::Mv => lyrics::OverlayStyle::Mv,
        };
        overlay_stack.push(Box::new(
            lyrics::LyricOverlay::from_subtitles(
                &path,
                width,
                height,
                &args.subtitle_font,
                args.subtitle_font_size,
                args.subtitle_fonts_dir.as_deref(),
                style,
            )
            .context("preparing lyric overlay")?,
        ));
    }
    if !args.no_title {
        let text = args.title.clone().unwrap_or_else(|| {
            args.input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
        // Empty/whitespace title (e.g. an input with no usable file stem) means
        // no title — skip the overlay rather than load a font to draw nothing.
        if text.trim().is_empty() {
            // ponytail: silently no-op; the user gets a video without a title.
        } else {
            // Title styling falls back to the subtitle styling when unset.
            let title_font = args.title_font.as_deref().unwrap_or(&args.subtitle_font);
            let title_font_size = args.title_font_size.unwrap_or(args.subtitle_font_size);
            let title_fonts_dir = args
                .title_fonts_dir
                .as_deref()
                .or(args.subtitle_fonts_dir.as_deref());
            overlay_stack.push(Box::new(
                lyrics::TitleOverlay::new(
                    &text,
                    width,
                    height,
                    title_font,
                    title_font_size,
                    title_fonts_dir,
                    args.title_duration,
                )
                .context("preparing title overlay")?,
            ));
        }
    }
    let mut overlays = lyrics::Overlays::new(overlay_stack);

    let shader_src = std::fs::read_to_string(&args.shader)
        .with_context(|| format!("reading shader {}", args.shader.display()))?;

    let mut renderer = render::Renderer::new(width, height, &shader_src)
        .context("initializing renderer")?;
    let mut encoder =
        encode::Encoder::new(&args.output, &args.input, width, height, args.fps)
            .context("starting ffmpeg encoder")?;

    for i in 0..total_frames {
        let t = i as f32 / args.fps as f32;
        let frame = renderer.render_frame(t, &analyzer.at(t));
        encoder.write_frame(overlays.composite(frame, t))?;
        if i % args.fps as usize == 0 {
            eprint!("\rrendering {}/{}", i, total_frames);
        }
    }
    eprintln!("\rrendered {} frames        ", total_frames);

    encoder.finish().context("finalizing video")?;
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

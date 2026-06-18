mod analysis;
mod audio;
mod encode;
mod lyrics;
mod render;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Turn an MP3 into an audio-reactive video driven by a GLSL fragment shader.
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

    /// Transcribe lyrics with whisper and burn them into the video as subtitles
    #[arg(long)]
    lyrics: bool,

    /// Whisper command to invoke for transcription
    #[arg(long, default_value = "whisper")]
    whisper_cmd: String,

    /// Whisper model to use (e.g. tiny, base, small, medium, large)
    #[arg(long, default_value = "medium")]
    whisper_model: String,

    /// Use an existing .srt subtitle file instead of transcribing
    #[arg(long)]
    subtitles: Option<PathBuf>,

    /// Print audio features per frame and exit (no rendering)
    #[arg(long)]
    dump_features: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (samples, sample_rate) = audio::decode(&args.input).context("decoding input audio")?;
    let duration = samples.len() as f32 / sample_rate as f32;
    let total_frames = (duration * args.fps as f32).ceil() as usize;
    eprintln!(
        "decoded {:.1}s @ {} Hz -> {} frames @ {} fps",
        duration, sample_rate, total_frames, args.fps
    );

    if args.dump_features {
        for i in 0..total_frames {
            let t = i as f32 / args.fps as f32;
            let f = analysis::analyze(&samples, sample_rate, t);
            println!(
                "{:6.3}s rms={:.4} bass={:.4} mid={:.4} treble={:.4}",
                t, f.rms, f.bass, f.mid, f.treble
            );
        }
        return Ok(());
    }

    // Resolve subtitles: an explicit .srt wins; otherwise transcribe if asked.
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

    let shader_src = std::fs::read_to_string(&args.shader)
        .with_context(|| format!("reading shader {}", args.shader.display()))?;

    let mut renderer = render::Renderer::new(args.width, args.height, &shader_src)
        .context("initializing renderer")?;
    let mut encoder = encode::Encoder::new(
        &args.output,
        &args.input,
        args.width,
        args.height,
        args.fps,
        subtitles.as_deref(),
    )
    .context("starting ffmpeg encoder")?;

    for i in 0..total_frames {
        let t = i as f32 / args.fps as f32;
        let features = analysis::analyze(&samples, sample_rate, t);
        let frame = renderer.render_frame(t, &features);
        encoder.write_frame(frame)?;
        if i % args.fps as usize == 0 {
            eprint!("\rrendering {}/{}", i, total_frames);
        }
    }
    eprintln!("\rrendered {} frames        ", total_frames);

    encoder.finish().context("finalizing video")?;
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

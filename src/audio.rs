use anyhow::{Context, Result, bail};
use std::path::Path;
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;

/// Decode an audio file to mono f32 samples. Returns (samples, sample_rate).
pub fn decode(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, Default::default(), Default::default())
        .context("probing audio format")?;

    let track = format
        .default_track(TrackType::Audio)
        .context("no default audio track")?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .context("track has no audio codec parameters")?;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .context("creating decoder")?;

    let mut samples: Vec<f32> = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    let mut sample_rate = 0u32;

    while let Some(packet) = format.next_packet().context("reading packet")? {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).context("decoding packet")?;
        let channels = push_mono(&decoded, &mut interleaved, &mut samples);
        sample_rate = decoded_rate(&decoded);
        debug_assert!(channels > 0);
    }

    if samples.is_empty() || sample_rate == 0 {
        bail!("decoded no audio samples from {}", path.display());
    }
    Ok((samples, sample_rate))
}

fn decoded_rate(buf: &GenericAudioBufferRef<'_>) -> u32 {
    buf.spec().rate()
}

/// Copy `buf` interleaved into `scratch`, then downmix to mono into `out`.
/// Returns the channel count.
fn push_mono(buf: &GenericAudioBufferRef<'_>, scratch: &mut Vec<f32>, out: &mut Vec<f32>) -> usize {
    let channels = buf.spec().channels().count().max(1);
    buf.copy_to_vec_interleaved::<f32>(scratch);
    for frame in scratch.chunks(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / channels as f32);
    }
    channels
}

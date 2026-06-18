use rustfft::{FftPlanner, num_complex::Complex};

const FFT_SIZE: usize = 2048;
pub const SPECTRUM_BINS: usize = 64;

/// Audio features for a single video frame.
pub struct Features {
    pub rms: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// `SPECTRUM_BINS` log-spaced magnitudes, normalized to ~0..1.
    pub spectrum: Vec<f32>,
}

/// Analyze a Hann-windowed FFT_SIZE window centered at `time` seconds.
pub fn analyze(samples: &[f32], sample_rate: u32, time: f32) -> Features {
    let center = (time * sample_rate as f32) as isize;
    let start = center - (FFT_SIZE as isize) / 2;

    // Gather windowed samples, zero-padding past the signal edges.
    let mut buf: Vec<Complex<f32>> = Vec::with_capacity(FFT_SIZE);
    let mut sum_sq = 0.0f32;
    for n in 0..FFT_SIZE {
        let idx = start + n as isize;
        let s = if idx >= 0 && (idx as usize) < samples.len() {
            samples[idx as usize]
        } else {
            0.0
        };
        sum_sq += s * s;
        // Hann window.
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / (FFT_SIZE as f32 - 1.0)).cos();
        buf.push(Complex::new(s * w, 0.0));
    }
    let rms = (sum_sq / FFT_SIZE as f32).sqrt();

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    fft.process(&mut buf);

    // Magnitude of the usable (non-mirrored) half.
    let half = FFT_SIZE / 2;
    let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
    let mags: Vec<f32> = buf[..half].iter().map(|c| c.norm()).collect();

    // Frequency bands (sum of magnitudes within Hz ranges, normalized by FFT size).
    let norm = FFT_SIZE as f32;
    let band = |lo: f32, hi: f32| -> f32 {
        let a = (lo / bin_hz).floor() as usize;
        let b = ((hi / bin_hz).ceil() as usize).min(half);
        mags[a.min(half)..b.max(a.min(half))].iter().sum::<f32>() / norm
    };
    let bass = band(20.0, 250.0);
    let mid = band(250.0, 4000.0);
    let treble = band(4000.0, 20000.0);

    // Log-spaced spectrum bins from ~20 Hz to Nyquist.
    let nyquist = sample_rate as f32 / 2.0;
    let f_min = 20.0f32.max(bin_hz);
    let mut spectrum = vec![0.0f32; SPECTRUM_BINS];
    for (i, slot) in spectrum.iter_mut().enumerate() {
        let lo = f_min * (nyquist / f_min).powf(i as f32 / SPECTRUM_BINS as f32);
        let hi = f_min * (nyquist / f_min).powf((i + 1) as f32 / SPECTRUM_BINS as f32);
        let a = (lo / bin_hz).floor() as usize;
        let b = ((hi / bin_hz).ceil() as usize).max(a + 1).min(half);
        let slice = &mags[a.min(half - 1)..b.max(a.min(half - 1) + 1).min(half)];
        let avg = if slice.is_empty() {
            0.0
        } else {
            slice.iter().sum::<f32>() / slice.len() as f32
        };
        // Compress dynamic range so quiet detail is still visible.
        *slot = (avg / norm * 8.0).min(1.0);
    }

    Features {
        rms,
        bass,
        mid,
        treble,
        spectrum,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 440 Hz sine should put its energy in the mid band, not bass/treble.
    #[test]
    fn sine_440_lands_in_mid_band() {
        let sr = 44_100;
        let samples: Vec<f32> = (0..sr)
            .map(|n| (std::f32::consts::TAU * 440.0 * n as f32 / sr as f32).sin())
            .collect();
        let f = analyze(&samples, sr, 0.5);
        assert!(
            f.mid > f.bass,
            "mid {} should exceed bass {}",
            f.mid,
            f.bass
        );
        assert!(
            f.mid > f.treble,
            "mid {} should exceed treble {}",
            f.mid,
            f.treble
        );
        assert!(f.rms > 0.1, "rms {} should be substantial", f.rms);
    }
}

use super::*;

fn only_denoise(pct: u32) -> Processing {
    Processing {
        denoise_pct: pct,
        gate_margin_db: None,
        normalize: false,
        volume_db: 0,
    }
}

#[test]
fn test_audio_dsp_rnnoise_suppression() {
    let mut dsp = AudioDsp::new();
    let mut samples = vec![1000i16; 480 * 4];

    dsp.process(&mut samples, only_denoise(100));

    assert_eq!(samples.len(), 480 * 4);
    assert!(samples.iter().any(|&s| s != 0));
}

#[test]
fn test_audio_dsp_zero_strength_passthrough() {
    let mut dsp = AudioDsp::new();
    let mut samples = vec![1234i16; 480 * 2];
    let copy = samples.clone();

    dsp.process(&mut samples, only_denoise(0));

    assert_eq!(samples, copy);
}

/// Voiced speech stand-in: a ~130 Hz pulse train through two formant resonances, which RNNoise
/// passes through as voice.
fn voice_like(len: usize) -> Vec<i16> {
    let sr = 48_000.0f32;
    let resonator = |f: f32, bw: f32| {
        let r = (-std::f32::consts::PI * bw / sr).exp();
        (
            2.0 * r * (2.0 * std::f32::consts::PI * f / sr).cos(),
            -r * r,
        )
    };
    let ((a1, a2), (b1, b2)) = (resonator(700.0, 110.0), resonator(1200.0, 130.0));
    let (mut phase, mut ya, mut yb) = (0.0f32, [0.0f32; 2], [0.0f32; 2]);
    let mut out: Vec<f32> = (0..len)
        .map(|n| {
            let t = n as f32 / sr;
            phase += (130.0 + 8.0 * (2.0 * std::f32::consts::PI * 5.0 * t).sin()) / sr;
            let pulse = if phase >= 1.0 {
                phase -= 1.0;
                1.0
            } else {
                0.0
            };
            let a = pulse + a1 * ya[0] + a2 * ya[1];
            ya = [a, ya[0]];
            let b = a + b1 * yb[0] + b2 * yb[1];
            yb = [b, yb[0]];
            b
        })
        .collect();
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    out.iter_mut().for_each(|v| *v = *v / peak * 12_000.0);
    out.iter().map(|&v| v as i16).collect()
}

#[test]
fn test_partial_strength_keeps_the_voice_whole() {
    // RNNoise's output is one frame late. Blending it with the raw audio of the same moment
    // cancels parts of the voice (comb filtering). Lined up, a half-strength blend of a voice
    // RNNoise keeps is just that voice, one frame later.
    const FRAME: usize = 480;
    let input = voice_like(FRAME * 200);
    let mut output = input.clone();
    let mut dsp = AudioDsp::new();
    for chunk in output.chunks_mut(FRAME) {
        dsp.process(chunk, only_denoise(50));
    }

    let settled = FRAME * 100;
    let (mut err, mut sig) = (0.0f64, 0.0f64);
    for n in settled..input.len() {
        let expected = input[n - FRAME] as f64;
        err += (output[n] as f64 - expected).powi(2);
        sig += expected.powi(2);
    }
    let relative = (err / sig).sqrt();
    assert!(relative < 0.1, "voice changed by {:.0}%", relative * 100.0);
}

#[test]
fn test_the_gate_quiets_noise_between_words() {
    let mut dsp = AudioDsp::new();
    let mut noise = vec![60i16; 480 * 100];
    let gated = Processing {
        gate_margin_db: Some(8),
        ..only_denoise(0)
    };
    for chunk in noise.chunks_mut(480) {
        dsp.process(chunk, gated);
    }
    assert!(
        noise[noise.len() - 1].abs() <= 3,
        "noise left at {}",
        noise[noise.len() - 1]
    );
}

#[test]
fn test_volume_applies_with_normalization_off() {
    let mut dsp = AudioDsp::new();
    let mut samples = vec![1000i16; 480 * 20];
    let louder = Processing {
        volume_db: 6,
        ..only_denoise(0)
    };
    for chunk in samples.chunks_mut(480) {
        dsp.process(chunk, louder);
    }
    let last = samples[samples.len() - 1];
    assert!((1980..=2010).contains(&last), "+6 dB of 1000 gave {last}");
}

/// Speech stand-in: syllables of `voice_like` (180 ms on, 70 ms off, a 500 ms pause after every
/// fourth), each a little louder or softer than the last, with the voiced frames at `speech_db`
/// RMS on average, over a room at `room_db`. Returns the samples and which 480-sample frames are
/// voiced and which are in a long pause.
fn talk(seconds: f32, speech_db: f32, room_db: f32) -> (Vec<i16>, Vec<bool>, Vec<bool>) {
    const SR: usize = 48_000;
    let n = (seconds * SR as f32) as usize;
    let voice = voice_like(n);
    let mut envelope = vec![0.0f32; n];
    let (on, off, pause) = (SR * 180 / 1000, SR * 70 / 1000, SR / 2);
    let loudness = [1.0f32, 0.6, 0.85, 0.5, 0.95, 0.7];
    let (mut at, mut syllable) = (0, 0);
    while at < n {
        let amp = loudness[syllable % loudness.len()];
        for i in 0..on.min(n - at) {
            // 5 ms fades, so syllables start and end like voices do.
            let fade = (i.min(on - 1 - i) as f32 / 240.0).min(1.0);
            envelope[at + i] = amp * fade;
        }
        at += on + if syllable % 4 == 3 { pause } else { off };
        syllable += 1;
    }
    let shaped: Vec<f32> = voice
        .iter()
        .zip(&envelope)
        .map(|(&v, &e)| v as f32 * e)
        .collect();
    let voiced: Vec<bool> = envelope
        .chunks(480)
        .map(|c| c.iter().all(|&e| e > 0.0))
        .collect();
    let paused: Vec<bool> = envelope
        .chunks(480)
        .map(|c| c.iter().all(|&e| e == 0.0))
        .collect();
    let voiced_rms = rms_db_where(&shaped, &voiced);
    let scale = 10.0f32.powf((speech_db - voiced_rms) / 20.0);
    // The room: white noise from a fixed seed, the same at any distance from the phone.
    let room = 32768.0 * 10.0f32.powf(room_db / 20.0) * 3.0f32.sqrt();
    let mut seed = 0x2545_f491u32;
    let samples = shaped
        .iter()
        .map(|&s| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            (s * scale + noise * room).clamp(-32768.0, 32767.0) as i16
        })
        .collect();
    (samples, voiced, paused)
}

/// The RMS in dBFS of the 480-sample frames marked in `which`.
fn rms_db_where(samples: &[f32], which: &[bool]) -> f32 {
    let (mut sum, mut count) = (0.0f64, 0usize);
    for (frame, &take) in samples.chunks(480).zip(which) {
        if take {
            sum += frame.iter().map(|&s| (s as f64).powi(2)).sum::<f64>();
            count += frame.len();
        }
    }
    (10.0 * (sum / count.max(1) as f64 / 32768.0f64.powi(2)).log10()) as f32
}

fn leveled() -> Processing {
    Processing {
        normalize: true,
        ..only_denoise(0)
    }
}

/// Runs `samples` through the processing in 10 ms frames, as the jitter buffer does.
fn run(dsp: &mut AudioDsp, samples: &mut [i16], p: Processing) {
    for chunk in samples.chunks_mut(480) {
        dsp.process(chunk, p);
    }
}

/// The level of the voiced frames from `from_s` seconds on.
fn voiced_level(samples: &[i16], voiced: &[bool], from_s: f32) -> f32 {
    let skip = (from_s * 100.0) as usize;
    let tail: Vec<f32> = samples[skip * 480..].iter().map(|&s| s as f32).collect();
    rms_db_where(&tail, &voiced[skip..])
}

#[test]
fn near_and_far_voices_come_out_equally_loud() {
    let mut levels = Vec::new();
    for speech_db in [-8.0, -25.0, -45.0] {
        let (mut samples, voiced, _) = talk(6.0, speech_db, -72.0);
        run(&mut AudioDsp::new(), &mut samples, leveled());
        levels.push(voiced_level(&samples, &voiced, 2.0));
    }
    let (lo, hi) = levels
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(hi - lo <= 2.0, "near to far came out at {levels:?}");
    assert!(
        levels.iter().all(|l| (-26.0..=-16.0).contains(l)),
        "not at the target: {levels:?}"
    );
}

#[test]
fn moving_from_far_to_near_settles_within_a_second() {
    let (mut far, voiced_far, _) = talk(4.0, -45.0, -72.0);
    let (mut near, voiced_near, _) = talk(4.0, -8.0, -72.0);
    let mut dsp = AudioDsp::new();
    run(&mut dsp, &mut far, leveled());
    let far_level = voiced_level(&far, &voiced_far, 2.0);
    run(&mut dsp, &mut near, leveled());
    let near_level = voiced_level(&near, &voiced_near, 1.0);
    assert!(
        (near_level - far_level).abs() <= 2.0,
        "far at {far_level}, a second after coming close at {near_level}"
    );
    let loudest = near.iter().map(|&s| (s as i32).abs()).max().unwrap();
    assert!(loudest < 32_767, "the step in distance clipped");
}

#[test]
fn the_room_does_not_swell_up_in_the_pauses() {
    let (mut samples, _, paused) = talk(6.0, -45.0, -62.0);
    run(&mut AudioDsp::new(), &mut samples, leveled());
    // The last whole long pause: its first and last 100 ms sound the same, the gain held.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (f, &p) in paused.iter().enumerate() {
        match runs.last_mut() {
            Some((_, end)) if p && *end == f => *end = f + 1,
            _ if p => runs.push((f, f + 1)),
            _ => {}
        }
    }
    let (first, end) = *runs
        .iter()
        .rev()
        .find(|(a, b)| b - a >= 40 && *b < paused.len())
        .expect("a long pause");
    let all: Vec<f32> = samples.iter().map(|&s| s as f32).collect();
    let level = |from: usize| {
        let which: Vec<bool> = (0..paused.len())
            .map(|f| (from..from + 10).contains(&f))
            .collect();
        rms_db_where(&all, &which)
    };
    let (start, finish) = (level(first), level(end - 10));
    assert!(
        (finish - start).abs() <= 1.5,
        "the room went from {start} to {finish} dBFS across a pause"
    );
}

#[test]
fn auto_level_off_leaves_the_voice_as_it_came() {
    let (mut samples, _, _) = talk(2.0, -40.0, -72.0);
    let copy = samples.clone();
    run(&mut AudioDsp::new(), &mut samples, only_denoise(0));
    assert_eq!(samples, copy);
}

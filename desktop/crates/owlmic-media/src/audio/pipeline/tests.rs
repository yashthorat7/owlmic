use super::*;

#[test]
fn test_jitter_buffer_prebuffering_and_levels() {
    let jb = JitterBuffer::new();
    assert_eq!(jb.target_samples(), USB_TARGET_MS * SAMPLES_PER_MS);

    jb.set_level(3);
    assert_eq!(jb.target_samples(), WIFI_TARGET_MS * SAMPLES_PER_MS);

    jb.set_level(4);
    assert_eq!(jb.target_samples(), BT_TARGET_MS * SAMPLES_PER_MS);

    jb.set_level(1);
    assert_eq!(jb.target_samples(), USB_TARGET_MS * SAMPLES_PER_MS);

    let mut out = [1.0f32; 480];
    let mut tone_10ms = [0i16; 480];
    for (i, s) in tone_10ms.iter_mut().enumerate() {
        let val = (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48000.0).sin();
        *s = (val * 4000.0) as i16;
    }

    jb.push_samples(&mut tone_10ms.clone()); // 10 ms
    jb.pop_samples(&mut out, 1);
    assert!(out.iter().all(|&v| v == 0.0)); // prebuffering not done

    jb.push_samples(&mut tone_10ms.clone()); // reaches 20 ms
    jb.pop_samples(&mut out, 1);
    // Output must be non-zero after prebuffer
    assert!(out.iter().any(|&v| v.abs() > 0.0));
}

#[test]
fn test_jitter_buffer_max_cap() {
    let jb = JitterBuffer::new();
    jb.push_samples(&mut vec![500i16; MAX_SAMPLES + 1000]);
    let mut out = [0.0f32; 48];
    jb.pop_samples(&mut out, 1);
    assert!(jb.len() <= MAX_SAMPLES, "the oldest audio past 200 ms went");
}

#[test]
fn test_reset_drops_queued_audio() {
    let jb = JitterBuffer::new();
    jb.push_samples(&mut [1000i16; 4800]);
    jb.reset();
    assert_eq!(jb.len(), 0);
    let mut out = [1.0f32; 480];
    jb.pop_samples(&mut out, 1);
    assert!(out.iter().all(|&v| v == 0.0), "playback starts over");
}

#[test]
fn test_drift_resampling_operation() {
    let jb = JitterBuffer::new();
    jb.set_level(1);
    let mut excess_samples = vec![1000i16; USB_TARGET_MS * SAMPLES_PER_MS + 2000];
    jb.push_samples(&mut excess_samples);

    let mut out = [0.0f32; 480];
    jb.pop_samples(&mut out, 1);
    assert!(jb.len() < excess_samples.len());
}

#[test]
fn test_output_resampled_to_device_rate() {
    // A 44.1 kHz device must still take the phone's 48 kHz at full speed, or the voice plays
    // slow and low and the buffer overflows. 10 ms there is 441 frames and 480 phone samples.
    let jb = JitterBuffer::new();
    jb.set_output_rate(44_100);
    jb.push_samples(&mut [1000i16; 4800]);
    let before = jb.len();
    let mut out = [0f32; 441];
    jb.pop_samples(&mut out, 1);
    let used = before - jb.len();
    assert!((478..=483).contains(&used), "used {} samples", used);
}

/// Plays a 12 kHz tone (the range of "s" sounds) through the buffer the way a USB link feeds
/// it: a 10 ms packet every 10 ms by the phone's clock, which runs 50 ppm off the PC's, each
/// landing 0 to 8 ms late, pulled by a 48 kHz device every 10 ms. Returns the tone's level in
/// each 2 ms of output after the first second.
fn jittery_tone_levels() -> Vec<f64> {
    let jb = JitterBuffer::new();
    jb.set_level(1);
    jb.set_noise(0, None); // RNNoise would take a steady tone for noise
    jb.set_auto_level(false);
    let mut tone: Vec<i16> = (0..48_000 * 5)
        .map(|n| {
            ((2.0 * std::f64::consts::PI * 12_000.0 * n as f64 / 48_000.0).sin() * 8000.0) as i16
        })
        .collect();
    let mut seed = 12_345u64;
    let mut lands_at = |k: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        k as f64 * 10.0 * (1.0 + 50e-6) + (seed >> 33) as f64 / (1u64 << 31) as f64 * 8.0
    };
    let (mut sent, mut next) = (0, lands_at(0));
    let mut out = Vec::new();
    for pull in 0..400 {
        let now_ms = pull as f64 * 10.0 + 3.7;
        while next <= now_ms {
            jb.push_samples(&mut tone[sent * 480..(sent + 1) * 480]);
            sent += 1;
            next = lands_at(sent);
        }
        let mut chunk = [0.0f32; 480];
        jb.pop_samples(&mut chunk, 1);
        out.extend_from_slice(&chunk);
    }
    out[48_000..]
        .chunks(96)
        .map(|c| (c.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt())
        .collect()
}

#[test]
fn test_high_frequencies_stay_steady_under_jitter() {
    // Drift correction used to steer by the raw buffer depth, which jumps a packet at a time,
    // so the speed slammed between its limits, and straight-line interpolation turned that into
    // a 12 kHz flutter of about 0.75 dB from one 2 ms to the next here. Now it is about 0.1 dB.
    let levels = jittery_tone_levels();
    assert!(levels.iter().all(|&l| l > 0.01), "playback went silent");
    let flutter_db = levels
        .windows(2)
        .map(|w| (20.0 * (w[1] / w[0]).log10()).abs())
        .sum::<f64>()
        / (levels.len() - 1) as f64;
    assert!(
        flutter_db < 0.2,
        "12 kHz level flutters {:.2} dB per 2 ms",
        flutter_db
    );
}

#[test]
fn test_gaps_fade_instead_of_clicking() {
    let jb = JitterBuffer::new();
    jb.set_level(1);
    jb.set_noise(0, None);
    jb.set_auto_level(false);
    jb.push_samples(&mut [16_000i16; USB_TARGET_MS * SAMPLES_PER_MS]);
    let mut out = [0.0f32; 1_440]; // more than the buffer holds
    jb.pop_samples(&mut out, 1);

    assert!(out[0].abs() < 0.01, "playback should fade in");
    assert!(out.iter().any(|&s| s > 0.45), "full level in between");
    let biggest_step = out
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(biggest_step < 0.01, "jump of {} is a click", biggest_step);
    assert_eq!(*out.last().unwrap(), 0.0);
}

#[test]
fn test_soft_clip_rounds_off_peaks() {
    use super::resample::soft_clip;
    assert_eq!(soft_clip(0.5), 0.5);
    assert_eq!(soft_clip(-SOFT_CLIP_KNEE), -SOFT_CLIP_KNEE);
    let mut last = SOFT_CLIP_KNEE;
    for x in [0.96f32, 1.0, 1.2, 2.0, 10.0] {
        let y = soft_clip(x);
        assert!(y >= last && y <= 1.0, "soft_clip({}) = {}", x, y);
        assert_eq!(soft_clip(-x), -y);
        last = y;
    }
    assert!(
        soft_clip(1.0) < 1.0,
        "a peak just over full scale is rounded, not flattened"
    );
    // No kink at the knee: just above it the curve still rises at about 1:1.
    let slope = (soft_clip(SOFT_CLIP_KNEE + 0.001) - SOFT_CLIP_KNEE) / 0.001;
    assert!((slope - 1.0).abs() < 0.05, "slope {} at the knee", slope);
}

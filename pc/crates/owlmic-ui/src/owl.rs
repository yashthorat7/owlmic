//! The tray owl (SYSTEM_DESIGN section 34), drawn in code: a round head with ear tufts, two
//! eyes and a beak cut out. One colour on transparent, white for dark taskbars and black for
//! light ones.

/// Coverage of the owl at (x, y) in a 16 x 16 box: 1 inside, 0 outside.
fn inside(x: f32, y: f32) -> bool {
    let d = |cx: f32, cy: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    let head = d(8.0, 9.0) <= 6.5;
    // Ear tufts: triangles rising from the head's top corners.
    let ear = |tip_x: f32, inner_x: f32| {
        let (lo, hi) = if tip_x < inner_x {
            (tip_x, inner_x)
        } else {
            (inner_x, tip_x)
        };
        (1.5..=6.0).contains(&y)
            && (lo..=hi).contains(&x)
            && (y - 1.5) >= 4.5 * (x - tip_x).abs() / (hi - lo) - 0.01
    };
    let eye_ring = |cx: f32| d(cx, 8.0) <= 2.6;
    let pupil = |cx: f32| d(cx, 8.3) <= 1.1;
    let beak = (10.0..=12.5).contains(&y) && (x - 8.0).abs() <= (12.5 - y) * 0.45;
    let body = head || ear(2.2, 5.5) || ear(13.8, 10.5);
    let hole = (eye_ring(5.3) && !pupil(5.3)) || (eye_ring(10.7) && !pupil(10.7)) || beak;
    body && !hole
}

/// The owl as BGRA pixels with straight alpha (as icons take them), `size` x `size`,
/// antialiased by 4 x 4 supersampling.
pub fn bgra(size: usize, white: bool) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let scale = 16.0 / size as f32;
    let shade = if white { 255 } else { 0 };
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = (px as f32 + (sx as f32 + 0.5) / 4.0) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / 4.0) * scale;
                    hits += inside(x, y) as u32;
                }
            }
            let alpha = (hits * 255 / 16) as u8;
            out[(py * size + px) * 4..][..4].copy_from_slice(&[shade, shade, shade, alpha]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owl_has_a_body_eyes_and_clear_corners() {
        let px = bgra(16, true);
        let alpha = |x: usize, y: usize| px[(y * 16 + x) * 4 + 3];
        assert_eq!(alpha(0, 15), 0, "corner");
        assert_eq!(alpha(8, 14), 255, "chin");
        assert_eq!(alpha(3, 8), 0, "inside the left eye ring");
        assert_eq!(alpha(5, 8), 255, "left pupil");
    }
}

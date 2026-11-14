//! The tray owl (SYSTEM_DESIGN section 34):
//! head contour with ear tufts, two eye cutout rings, and two inner pupil dots.
//! One colour on transparent, white for dark taskbars and black for light ones.

const CONTOUR: [(f32, f32); 48] = [
    (50.0, 24.0),
    (44.89, 23.64),
    (40.06, 22.66),
    (35.52, 21.15),
    (31.25, 19.25),
    (27.27, 17.07),
    (23.56, 14.72),
    (20.14, 12.32),
    (17.0, 10.0),
    (15.6, 14.67),
    (14.41, 19.62),
    (13.45, 24.77),
    (12.75, 30.0),
    (12.33, 35.23),
    (12.22, 40.38),
    (12.43, 45.33),
    (13.0, 50.0),
    (14.71, 56.63),
    (17.52, 62.91),
    (21.28, 68.67),
    (25.88, 73.75),
    (31.17, 77.99),
    (37.05, 81.22),
    (43.37, 83.28),
    (50.0, 84.0),
    (56.63, 83.28),
    (62.95, 81.22),
    (68.83, 77.99),
    (74.12, 73.75),
    (78.72, 68.67),
    (82.48, 62.91),
    (85.29, 56.63),
    (87.0, 50.0),
    (87.57, 45.33),
    (87.78, 40.38),
    (87.67, 35.23),
    (87.25, 30.0),
    (86.55, 24.77),
    (85.59, 19.62),
    (84.4, 14.67),
    (83.0, 10.0),
    (79.86, 12.32),
    (76.44, 14.72),
    (72.73, 17.07),
    (68.75, 19.25),
    (64.48, 21.15),
    (59.94, 22.66),
    (55.11, 23.64),
];

/// Coverage of the owl at (x, y) in a 16 x 16 box: true inside, false outside.
fn inside(x: f32, y: f32) -> bool {
    let sx = x * (100.0 / 16.0);
    let sy = y * (100.0 / 16.0);

    let mut in_body = false;
    let mut j = CONTOUR.len() - 1;
    for (i, &(xi, yi)) in CONTOUR.iter().enumerate() {
        let (xj, yj) = CONTOUR[j];
        if (yi > sy) != (yj > sy) && sx < (xj - xi) * (sy - yi) / (yj - yi) + xi {
            in_body = !in_body;
        }
        j = i;
    }
    if !in_body {
        return false;
    }

    for &(ex, ey) in &[(34.0, 49.0), (66.0, 49.0)] {
        let d = ((sx - ex).powi(2) + (sy - ey).powi(2)).sqrt();
        if d <= 13.5 && d > 7.5 {
            return false;
        }
    }
    true
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
        assert_eq!(alpha(8, 12), 255, "chin");
        assert_eq!(alpha(5, 9), 0, "inside the eye ring cutout");
        assert_eq!(alpha(5, 7), 255, "left pupil");
        assert_eq!(alpha(10, 7), 255, "right pupil");
    }
}

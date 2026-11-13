/// A frame in BGR order, 3 bytes per pixel: what softcam takes as is.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub bgr: Vec<u8>,
}

impl Frame {
    pub fn new(width: usize, height: usize, bgr: Vec<u8>) -> Self {
        Self { width, height, bgr }
    }

    /// Scales this frame to fit `dst_w` x `dst_h` into `out`, keeping its shape, with black bars
    /// where it doesn't fill. Bilinear, so a 720p picture scaled up to 1080p stays smooth
    /// instead of blocky. `out` is reused, so a steady stream allocates nothing.
    pub fn letterbox_into(&self, dst_w: usize, dst_h: usize, out: &mut Vec<u8>) {
        out.clear();
        out.resize(dst_w * dst_h * 3, 0);

        let scale = (dst_w as f32 / self.width as f32).min(dst_h as f32 / self.height as f32);
        let scaled_w = ((self.width as f32 * scale).round() as usize).clamp(1, dst_w);
        let scaled_h = ((self.height as f32 * scale).round() as usize).clamp(1, dst_h);
        let (offset_x, offset_y) = ((dst_w - scaled_w) / 2, (dst_h - scaled_h) / 2);

        // Byte offsets of the two source pixels behind each output column, and the second's weight.
        let columns: Vec<(usize, usize, u32)> = (0..scaled_w)
            .map(|dx| {
                let (sx, wx) = sample_at(dx, scaled_w, self.width);
                (sx * 3, (sx + 1).min(self.width - 1) * 3, wx)
            })
            .collect();
        let widen = |src: &[u8], dst: &mut [u32]| {
            for (d, &(a, b, w)) in dst.as_chunks_mut::<3>().0.iter_mut().zip(&columns) {
                for c in 0..3 {
                    d[c] = src[a + c] as u32 * (256 - w) + src[b + c] as u32 * w;
                }
            }
        };

        // The two source rows an output row blends, already scaled across. Output rows go down in
        // order and neighbours mostly share source rows, so each is scaled across about once.
        let stride = self.width * 3;
        let (mut top, mut bottom) = (vec![0u32; scaled_w * 3], vec![0u32; scaled_w * 3]);
        let (mut top_y, mut bottom_y) = (usize::MAX, usize::MAX);
        for dy in 0..scaled_h {
            let (sy, wy) = sample_at(dy, scaled_h, self.height);
            let sy_next = (sy + 1).min(self.height - 1);
            if top_y != sy {
                if bottom_y == sy {
                    std::mem::swap(&mut top, &mut bottom);
                    bottom_y = usize::MAX;
                } else {
                    widen(&self.bgr[sy * stride..][..stride], &mut top);
                }
                top_y = sy;
            }
            if bottom_y != sy_next {
                widen(&self.bgr[sy_next * stride..][..stride], &mut bottom);
                bottom_y = sy_next;
            }
            let row = &mut out[((offset_y + dy) * dst_w + offset_x) * 3..][..scaled_w * 3];
            for ((px, &t), &b) in row.iter_mut().zip(&top).zip(&bottom) {
                *px = ((t * (256 - wy) + b * wy + (1 << 15)) >> 16) as u8;
            }
        }
    }

    /// Generates a neutral placeholder frame for when the camera is off:
    /// Black background with a subtle, clean Owlmic mark in the center (#3A3A3C / #8E8E93).
    pub fn placeholder(width: usize, height: usize) -> Self {
        let mut bgr = vec![0x11u8; width * height * 3]; // dark surface #111111

        let center_x = width / 2;
        let center_y = height / 2;
        let mark_size = (width.min(height) / 8).max(12);

        // Draw a minimalist camera glyph outline in #8E8E93 (142, 142, 147)
        let x_start = center_x.saturating_sub(mark_size);
        let x_end = (center_x + mark_size).min(width);
        let y_start = center_y.saturating_sub(mark_size * 2 / 3);
        let y_end = (center_y + mark_size * 2 / 3).min(height);

        for y in y_start..y_end {
            for x in x_start..x_end {
                let border = x == x_start || x == x_end - 1 || y == y_start || y == y_end - 1;
                let dx = x as isize - center_x as isize;
                let dy = y as isize - center_y as isize;
                let dist_sq = dx * dx + dy * dy;
                let inner_radius = (mark_size / 3) as isize;
                let lens = (dist_sq - inner_radius * inner_radius).abs() <= mark_size as isize;

                if border || lens {
                    let idx = (y * width + x) * 3;
                    bgr[idx] = 0x93; // B
                    bgr[idx + 1] = 0x8E; // G
                    bgr[idx + 2] = 0x8E; // R
                }
            }
        }

        Frame { width, height, bgr }
    }
}

/// Maps pixel `d` of `dst_len` onto a source `src_len` long, by pixel centers: the source pixel
/// at or before it, and how much (0 to 256) the next one counts.
fn sample_at(d: usize, dst_len: usize, src_len: usize) -> (usize, u32) {
    let pos = ((d as f32 + 0.5) * src_len as f32 / dst_len as f32 - 0.5).max(0.0);
    let i = (pos as usize).min(src_len - 1);
    (i, ((pos - i as f32) * 256.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize, value: u8) -> Frame {
        Frame::new(width, height, vec![value; width * height * 3])
    }

    #[test]
    fn test_letterbox_same_size_is_unchanged() {
        let frame = Frame::new(4, 2, (0..24).collect());
        let mut out = Vec::new();
        frame.letterbox_into(4, 2, &mut out);
        assert_eq!(out, frame.bgr);
    }

    #[test]
    fn test_letterbox_720p_fills_1080p() {
        let mut out = Vec::new();
        solid(1280, 720, 200).letterbox_into(1920, 1080, &mut out);
        assert_eq!(out.len(), 1920 * 1080 * 3);
        assert!(
            out.iter().all(|&v| v == 200),
            "same shape: no bars, no seams"
        );
    }

    #[test]
    fn test_letterbox_4_3_gets_side_bars() {
        let mut out = Vec::new();
        solid(960, 720, 200).letterbox_into(1920, 1080, &mut out);
        // 960x720 scales to 1440x1080, centred: 240 black columns each side.
        let pixel = |x: usize, y: usize| out[(y * 1920 + x) * 3];
        for y in [0, 540, 1079] {
            assert_eq!(pixel(0, y), 0);
            assert_eq!(pixel(239, y), 0);
            assert_eq!(pixel(240, y), 200);
            assert_eq!(pixel(1679, y), 200);
            assert_eq!(pixel(1680, y), 0);
        }
    }

    #[test]
    fn test_letterbox_upscale_is_smooth() {
        // Nearest-pixel scaling turned [0, 255] into [0, 0, 255, 255]; blending gives steps between.
        let frame = Frame::new(2, 1, vec![0, 0, 0, 255, 255, 255]);
        let mut out = Vec::new();
        frame.letterbox_into(4, 2, &mut out);
        let row: Vec<u8> = out[..12].chunks(3).map(|p| p[0]).collect();
        assert_eq!(row, [0, 64, 191, 255]);
    }
}

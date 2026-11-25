//! NV12 pictures: what the H.264 decoder gives and Owlmic Cam serves on Windows 11. Shaping
//! (Fill or Fit, Mirror) and scaling to the fixed 1920 x 1080 output happen here
//! (SYSTEM_DESIGN section 17.2).

/// Limited-range black.
const BLACK_Y: u8 = 16;
const NEUTRAL_UV: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// Crops to fill the output.
    Fill,
    /// Shows the whole picture with black bars.
    Fit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nv12 {
    pub width: usize,
    pub height: usize,
    /// The Y plane, then interleaved U and V at half resolution.
    pub data: Vec<u8>,
}

impl Nv12 {
    pub fn black(width: usize, height: usize) -> Self {
        let mut data = vec![BLACK_Y; width * height];
        data.resize(width * height * 3 / 2, NEUTRAL_UV);
        Self {
            width,
            height,
            data,
        }
    }

    pub fn len_for(width: usize, height: usize) -> usize {
        width * height * 3 / 2
    }

    fn y(&self) -> &[u8] {
        &self.data[..self.width * self.height]
    }

    fn uv(&self) -> &[u8] {
        &self.data[self.width * self.height..]
    }

    /// Scales this picture into `dst` (whose size stays), cropped or letterboxed, optionally
    /// mirrored. Bilinear for brightness, nearest for colour; `dst` is reused.
    pub fn shape_into(&self, dst: &mut Nv12, framing: Framing, mirror: bool) {
        Nv12Shaper::new().shape_into(self, dst, framing, mirror);
    }

    /// BGR, 3 bytes a pixel, top row first: what softcam takes on Windows 10.
    pub fn to_bgr(&self, out: &mut Vec<u8>) {
        out.resize(self.width * self.height * 3, 0);
        let (y, uv) = (self.y(), self.uv());
        for row in 0..self.height {
            for col in 0..self.width {
                let yy = (y[row * self.width + col] as i32 - 16).max(0) * 298;
                let k = (row / 2) * self.width + (col / 2) * 2;
                let (u, v) = (uv[k] as i32 - 128, uv[k + 1] as i32 - 128);
                let r = (yy + 409 * v + 128) >> 8;
                let g = (yy - 100 * u - 208 * v + 128) >> 8;
                let b = (yy + 516 * u + 128) >> 8;
                let o = (row * self.width + col) * 3;
                out[o] = b.clamp(0, 255) as u8;
                out[o + 1] = g.clamp(0, 255) as u8;
                out[o + 2] = r.clamp(0, 255) as u8;
            }
        }
    }

    /// A small BGRA copy (nearest pixel), for the panel's preview, optionally mirrored.
    pub fn to_bgra_scaled(&self, width: usize, height: usize, mirror: bool, out: &mut [u8]) {
        let (y, uv) = (self.y(), self.uv());
        for row in 0..height {
            let sy = row * self.height / height;
            for col in 0..width {
                let sx = if mirror {
                    self.width.saturating_sub(1 + col * self.width / width)
                } else {
                    col * self.width / width
                };
                let yy = (y[sy * self.width + sx] as i32 - 16).max(0) * 298;
                let k = (sy / 2) * self.width + (sx / 2) * 2;
                let (u, v) = (uv[k] as i32 - 128, uv[k + 1] as i32 - 128);
                let o = (row * width + col) * 4;
                out[o] = ((yy + 516 * u + 128) >> 8).clamp(0, 255) as u8;
                out[o + 1] = ((yy - 100 * u - 208 * v + 128) >> 8).clamp(0, 255) as u8;
                out[o + 2] = ((yy + 409 * v + 128) >> 8).clamp(0, 255) as u8;
                out[o + 3] = 255;
            }
        }
    }

    /// From BGRA pixels, top row first: for the placeholder picture drawn on the PC.
    pub fn from_bgra(width: usize, height: usize, bgra: &[u8]) -> Self {
        let mut f = Nv12::black(width, height);
        let (y, uv) = f.data.split_at_mut(width * height);
        for row in 0..height {
            for col in 0..width {
                let p = &bgra[(row * width + col) * 4..];
                let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
                y[row * width + col] = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16) as u8;
                if row % 2 == 0 && col % 2 == 0 {
                    let k = (row / 2) * width + col;
                    uv[k] = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128) as u8;
                    uv[k + 1] = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128) as u8;
                }
            }
        }
        f
    }
}

/// Reusable NV12 scaler/shaper that caches LUTs across frames for zero per-frame allocations.
#[derive(Default)]
pub struct Nv12Shaper {
    key: Option<(usize, usize, usize, usize, Framing, bool)>,
    window: (usize, usize, usize, usize, usize, usize, usize, usize),
    x_lut: Vec<(usize, usize, u32, usize)>,
    y_lut: Vec<(usize, usize, u32)>,
    uv_x_lut: Vec<(usize, usize)>,
    uv_y_lut: Vec<usize>,
}

impl Nv12Shaper {
    pub fn new() -> Self {
        Self::default()
    }

    fn prepare(
        &mut self,
        sw: usize,
        sh: usize,
        dw: usize,
        dh: usize,
        framing: Framing,
        mirror: bool,
    ) {
        let key = (sw, sh, dw, dh, framing, mirror);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let w = place(sw, sh, dw, dh, framing);
        self.window = w;
        let (sx, sy, cw, ch, ox, _oy, ow, oh) = w;
        if cw == ow && ch == oh {
            self.x_lut.clear();
            self.y_lut.clear();
            self.uv_x_lut.clear();
            self.uv_y_lut.clear();
            return;
        }
        self.x_lut.clear();
        self.x_lut.extend((0..ow).map(|ox_i| {
            let fx = (ox_i as f32 + 0.5) * cw as f32 / ow as f32 - 0.5;
            let x0 = (fx.max(0.0) as usize).min(cw - 1);
            let x1 = (x0 + 1).min(cw - 1);
            let wx = ((fx - x0 as f32).clamp(0.0, 1.0) * 256.0) as u32;
            let dx = if mirror {
                dw - 1 - (ox + ox_i)
            } else {
                ox + ox_i
            };
            (sx + x0, sx + x1, wx, dx)
        }));
        self.y_lut.clear();
        self.y_lut.extend((0..oh).map(|oy_i| {
            let fy = (oy_i as f32 + 0.5) * ch as f32 / oh as f32 - 0.5;
            let y0 = (fy.max(0.0) as usize).min(ch - 1);
            let y1 = (y0 + 1).min(ch - 1);
            let wy = ((fy - y0 as f32).clamp(0.0, 1.0) * 256.0) as u32;
            (sy + y0, sy + y1, wy)
        }));
        let (ow2, oh2, ox2) = (ow / 2, oh / 2, ox / 2);
        self.uv_x_lut.clear();
        self.uv_x_lut.extend((0..ow2).map(|i| {
            let si = (sx / 2 + i * (cw / 2) / ow2.max(1)).min(sw / 2 - 1);
            let di = if mirror {
                dw / 2 - 1 - (ox2 + i)
            } else {
                ox2 + i
            };
            (si * 2, di * 2)
        }));
        self.uv_y_lut.clear();
        self.uv_y_lut
            .extend((0..oh2).map(|j| (sy / 2 + j * (ch / 2) / oh2.max(1)).min(sh / 2 - 1)));
    }

    pub fn shape_into(&mut self, src: &Nv12, dst: &mut Nv12, framing: Framing, mirror: bool) {
        let (dw, dh) = (dst.width, dst.height);
        let y_len = dw * dh;
        dst.data.resize(Nv12::len_for(dw, dh), 0);
        self.prepare(src.width, src.height, dw, dh, framing, mirror);
        let (sx, sy, cw, ch, ox, oy, ow, oh) = self.window;

        if !mirror
            && (src.width, src.height) == (dw, dh)
            && sx == 0
            && sy == 0
            && ox == 0
            && oy == 0
            && cw == dw
            && ch == dh
            && ow == dw
            && oh == dh
        {
            let total = Nv12::len_for(dw, dh);
            dst.data[..total].copy_from_slice(&src.data[..total]);
            return;
        }

        let (y_dst, uv_dst) = dst.data.split_at_mut(y_len);
        if ow < dw || oh < dh {
            y_dst.fill(BLACK_Y);
            uv_dst.fill(NEUTRAL_UV);
        }
        let src_y = src.y();
        let src_uv = src.uv();

        // Fast 1:1 unscaled path (e.g. 1080p mirrored or letterboxed without resampling).
        if cw == ow && ch == oh {
            if !mirror {
                for row in 0..oh {
                    let s = (sy + row) * src.width + sx;
                    let d = (oy + row) * dw + ox;
                    y_dst[d..d + ow].copy_from_slice(&src_y[s..s + ow]);
                }
                let (oh2, oy2, ox2, ow2) = (oh / 2, oy / 2, ox / 2, ow / 2);
                for row in 0..oh2 {
                    let s = (sy / 2 + row) * src.width + (sx & !1);
                    let d = (oy2 + row) * dw + ox2 * 2;
                    uv_dst[d..d + ow2 * 2].copy_from_slice(&src_uv[s..s + ow2 * 2]);
                }
            } else {
                for row in 0..oh {
                    let s_row = &src_y[(sy + row) * src.width + sx..][..ow];
                    let d_row = &mut y_dst[(oy + row) * dw + (dw - ox - ow)..][..ow];
                    for (d, &s) in d_row.iter_mut().rev().zip(s_row) {
                        *d = s;
                    }
                }
                let (oh2, oy2, ox2, ow2) = (oh / 2, oy / 2, ox / 2, ow / 2);
                for row in 0..oh2 {
                    let s_row = &src_uv[(sy / 2 + row) * src.width + (sx & !1)..][..ow2 * 2];
                    let d_row = &mut uv_dst[(oy2 + row) * dw + (dw - (ox2 + ow2) * 2)..][..ow2 * 2];
                    for (d_pair, s_pair) in d_row
                        .as_chunks_mut::<2>()
                        .0
                        .iter_mut()
                        .rev()
                        .zip(s_row.as_chunks::<2>().0.iter())
                    {
                        d_pair[0] = s_pair[0];
                        d_pair[1] = s_pair[1];
                    }
                }
            }
            return;
        }

        for (oy_i, &(y0, y1, wy)) in self.y_lut.iter().enumerate() {
            let w0 = 256 - wy;
            let row0 = &src_y[y0 * src.width..];
            let row1 = &src_y[y1 * src.width..];
            let out_row = &mut y_dst[(oy + oy_i) * dw..];
            for &(sx0, sx1, wx, dx) in &self.x_lut {
                let top = row0[sx0] as u32 * (256 - wx) + row0[sx1] as u32 * wx;
                let bottom = row1[sx0] as u32 * (256 - wx) + row1[sx1] as u32 * wx;
                out_row[dx] = ((top * w0 + bottom * wy) >> 16) as u8;
            }
        }
        let oy2 = oy / 2;
        for (j, &sj) in self.uv_y_lut.iter().enumerate() {
            let src_row = sj * src.width;
            let dst_row = (oy2 + j) * dw;
            for &(si2, di2) in &self.uv_x_lut {
                uv_dst[dst_row + di2] = src_uv[src_row + si2];
                uv_dst[dst_row + di2 + 1] = src_uv[src_row + si2 + 1];
            }
        }
    }
}

/// Source window (x, y, w, h) and output window (x, y, w, h), all even so colour rows line up.
#[allow(clippy::type_complexity)]
fn place(
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    framing: Framing,
) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let even = |v: usize| (v / 2 * 2).max(2);
    let src_wider = sw * dh > dw * sh;
    match (framing, src_wider) {
        (Framing::Fill, true) => {
            let w = even(sh * dw / dh);
            (even((sw - w) / 2).min(sw - w), 0, w, sh, 0, 0, dw, dh)
        }
        (Framing::Fill, false) => {
            let h = even(sw * dh / dw);
            (0, even((sh - h) / 2).min(sh - h), sw, h, 0, 0, dw, dh)
        }
        (Framing::Fit, true) => {
            let h = even(dw * sh / sw);
            (0, 0, sw, sh, 0, even((dh - h) / 2).min(dh - h), dw, h)
        }
        (Framing::Fit, false) => {
            let w = even(dh * sw / sh);
            (0, 0, sw, sh, even((dw - w) / 2).min(dw - w), 0, w, dh)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, y: u8) -> Nv12 {
        let mut f = Nv12::black(w, h);
        f.data[..w * h].fill(y);
        f
    }

    #[test]
    fn a_720p_picture_fills_1080p() {
        let mut out = Nv12::black(1920, 1080);
        solid(1280, 720, 200).shape_into(&mut out, Framing::Fill, false);
        assert!(out.data[..1920 * 1080].iter().all(|v| *v == 200));
    }

    #[test]
    fn a_portrait_picture_fits_with_side_bars_or_fills_by_cropping() {
        let mut out = Nv12::black(1920, 1080);
        solid(720, 1280, 200).shape_into(&mut out, Framing::Fit, false);
        assert_eq!(out.data[0], BLACK_Y, "bar on the left");
        assert_eq!(out.data[960], 200, "picture in the middle");
        solid(720, 1280, 200).shape_into(&mut out, Framing::Fill, false);
        assert_eq!(out.data[0], 200);
    }

    #[test]
    fn mirror_flips_left_and_right() {
        let mut src = Nv12::black(4, 2);
        src.data[0] = 235;
        src.data[4] = 235;
        let mut out = Nv12::black(4, 2);
        src.shape_into(&mut out, Framing::Fill, true);
        assert_eq!(out.data[3], 235);
        assert_eq!(out.data[0], BLACK_Y);
    }

    #[test]
    fn the_preview_is_a_scaled_opaque_copy() {
        let bgra: Vec<u8> = (0..64).flat_map(|_| [40u8, 120, 220, 255]).collect();
        let f = Nv12::from_bgra(8, 8, &bgra);
        let mut small = vec![0u8; 4 * 4 * 4];
        f.to_bgra_scaled(4, 4, false, &mut small);
        assert!((small[2] as i32 - 220).abs() <= 3);
        assert_eq!(small[3], 255);
    }

    #[test]
    fn colour_survives_a_trip_through_bgr() {
        let bgra: Vec<u8> = (0..16).flat_map(|_| [40u8, 120, 220, 255]).collect();
        let f = Nv12::from_bgra(4, 4, &bgra);
        let mut bgr = Vec::new();
        f.to_bgr(&mut bgr);
        for (got, want) in bgr[..3].iter().zip([40i32, 120, 220]) {
            assert!((*got as i32 - want).abs() <= 3, "{got} vs {want}");
        }
    }
}

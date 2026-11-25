//! Direct2D and DirectWrite drawing for the panel. Everything is placed in physical pixels, so
//! the 1 px hairlines stay one crisp pixel at every scale (SYSTEM_DESIGN section 29.1).

use crate::glyph::{BOX, Icon, Seg, Shape};
use crate::layout::Rect;
use crate::{TextStyle, text_style};
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{BOOL, HSTRING, Result};
use windows_numerics::{Matrix3x2, Vector2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    Title,
    Body,
    Caption,
    /// The approval code, in the monospaced face.
    #[allow(dead_code)]
    Code,
}

fn color_alpha(rgb: u32, alpha: f32) -> D2D1_COLOR_F {
    let c = |shift: u32| ((rgb >> shift) & 0xFF) as f32 / 255.0;
    D2D1_COLOR_F {
        r: c(16),
        g: c(8),
        b: c(0),
        a: alpha.clamp(0.0, 1.0),
    }
}

fn color(rgb: u32) -> D2D1_COLOR_F {
    color_alpha(rgb, 1.0)
}

/// Draws the Owlmic owl symbol centered at `(cx, cy)` with width and height `size` in physical pixels.
pub fn draw_brand_owl(
    d2d: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    cx: f32,
    cy: f32,
    size: f32,
) {
    let k = size / BOX;
    let (ox, oy) = (cx - size / 2.0, cy - size / 2.0);
    let p = move |x: f32, y: f32| Vector2 {
        X: ox + x * k,
        Y: oy + y * k,
    };
    unsafe {
        if let Ok(geometry) = d2d.CreatePathGeometry()
            && let Ok(sink) = geometry.Open()
        {
            sink.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            let mut open = false;
            for seg in crate::brand::OWL_BODY {
                match *seg {
                    Seg::Move(x, y) => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        }
                        sink.BeginFigure(p(x, y), D2D1_FIGURE_BEGIN_FILLED);
                        open = true;
                    }
                    Seg::Line(x, y) => sink.AddLine(p(x, y)),
                    Seg::Cubic(c) => sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: p(c[0], c[1]),
                        point2: p(c[2], c[3]),
                        point3: p(c[4], c[5]),
                    }),
                    Seg::Close if open => {
                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        open = false;
                    }
                    _ => {}
                }
            }
            if open {
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            let _ = sink.Close();
            target.FillGeometry(&geometry, brush, None);
        }
        for shape in crate::brand::OWL_PUPILS {
            if let Shape::Circle { c, r } = *shape {
                let ellipse = D2D1_ELLIPSE {
                    point: p(c.0, c.1),
                    radiusX: r * k,
                    radiusY: r * k,
                };
                target.FillEllipse(&ellipse, brush);
            }
        }
    }
}

struct Formats {
    scale: f32,
    title: IDWriteTextFormat,
    body: IDWriteTextFormat,
    caption: IDWriteTextFormat,
    code: IDWriteTextFormat,
}

pub struct Painter {
    d2d: ID2D1Factory,
    dwrite: IDWriteFactory,
    stroke: ID2D1StrokeStyle,
    hwnd_target: Option<ID2D1HwndRenderTarget>,
    dc_target: Option<ID2D1DCRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    formats: Option<Formats>,
    family: HSTRING,
    mono: HSTRING,
    preview: Option<(ID2D1Bitmap, u64)>,
    preview_ambient: Option<(ID2D1Bitmap, u64)>,
    layers: Vec<ID2D1Layer>,
    layer_depth: usize,
    pub scale: f32,
}

/// Geist when installed, else the Windows UI face.
fn pick_family(dwrite: &IDWriteFactory, choices: &[&str]) -> HSTRING {
    let mut fonts = None;
    unsafe {
        let _ = dwrite.GetSystemFontCollection(&mut fonts, false);
    }
    for name in choices {
        let (mut index, mut exists) = (0u32, BOOL(0));
        if let Some(f) = &fonts
            && unsafe { f.FindFamilyName(&HSTRING::from(*name), &mut index, &mut exists) }.is_ok()
            && exists.as_bool()
        {
            return HSTRING::from(*name);
        }
    }
    HSTRING::from(*choices.last().unwrap())
}

impl Painter {
    pub fn new() -> Result<Self> {
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let stroke = d2d.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 4.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            let family = pick_family(&dwrite, &["Geist", "Segoe UI Variable Text", "Segoe UI"]);
            let mono = pick_family(&dwrite, &["Geist Mono", "Cascadia Mono", "Consolas"]);
            Ok(Self {
                d2d,
                dwrite,
                stroke,
                hwnd_target: None,
                dc_target: None,
                brush: None,
                formats: None,
                family,
                mono,
                preview: None,
                preview_ambient: None,
                layers: Vec::new(),
                layer_depth: 0,
                scale: 1.0,
            })
        }
    }

    fn format(&self, family: &HSTRING, style: TextStyle) -> Result<IDWriteTextFormat> {
        let weight = if style.weight >= 600 {
            DWRITE_FONT_WEIGHT_SEMI_BOLD
        } else {
            DWRITE_FONT_WEIGHT_NORMAL
        };
        unsafe {
            let f = self.dwrite.CreateTextFormat(
                family,
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size_px * self.scale,
                &HSTRING::from("en-us"),
            )?;
            let baseline = (style.line_px + style.size_px * 0.72) * 0.5 * self.scale;
            f.SetLineSpacing(
                DWRITE_LINE_SPACING_METHOD_UNIFORM,
                style.line_px * self.scale,
                baseline,
            )?;
            Ok(f)
        }
    }

    /// Starts a frame for `hwnd` at `scale`. False if there is nothing to draw on.
    pub fn begin(&mut self, hwnd: HWND, scale: f32) -> bool {
        self.scale = scale;
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rc);
        }
        let size = D2D_SIZE_U {
            width: (rc.right - rc.left).max(1) as u32,
            height: (rc.bottom - rc.top).max(1) as u32,
        };
        if self.hwnd_target.is_none() {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: size,
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let Ok(target) = (unsafe { self.d2d.CreateHwndRenderTarget(&props, &hwnd_props) })
            else {
                return false;
            };
            self.brush = unsafe { target.CreateSolidColorBrush(&color(0xFFFFFF), None).ok() };
            self.hwnd_target = Some(target);
            self.preview = None;
            self.preview_ambient = None;
            self.layers.clear();
            self.layer_depth = 0;
        }
        self.layer_depth = 0;
        if self.formats.as_ref().is_none_or(|f| f.scale != scale) {
            let built = (|| {
                Ok::<_, windows::core::Error>(Formats {
                    scale,
                    title: self.format(&self.family, text_style::TITLE)?,
                    body: self.format(&self.family, text_style::BODY)?,
                    caption: self.format(&self.family, text_style::CAPTION)?,
                    code: self.format(&self.mono, text_style::DISPLAY)?,
                })
            })();
            self.formats = built.ok();
        }
        let target = self.hwnd_target.as_ref().unwrap();
        unsafe {
            let _ = target.Resize(&size);
            target.BeginDraw();
        }
        self.brush.is_some() && self.formats.is_some()
    }

    /// Starts an offscreen frame bound to GDI `hdc` within `rc`.
    #[allow(dead_code)]
    pub fn begin_dc(
        &mut self,
        hdc: windows::Win32::Graphics::Gdi::HDC,
        rc: &RECT,
        scale: f32,
    ) -> bool {
        self.scale = scale;
        if self.dc_target.is_none() {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let Ok(dc) = (unsafe { self.d2d.CreateDCRenderTarget(&props) }) else {
                return false;
            };
            self.brush = unsafe { dc.CreateSolidColorBrush(&color(0xFFFFFF), None).ok() };
            self.dc_target = Some(dc);
            self.preview = None;
            self.preview_ambient = None;
            self.layers.clear();
            self.layer_depth = 0;
        }
        self.layer_depth = 0;
        if self.formats.as_ref().is_none_or(|f| f.scale != scale) {
            let built = (|| {
                Ok::<_, windows::core::Error>(Formats {
                    scale,
                    title: self.format(&self.family, text_style::TITLE)?,
                    body: self.format(&self.family, text_style::BODY)?,
                    caption: self.format(&self.family, text_style::CAPTION)?,
                    code: self.format(&self.mono, text_style::DISPLAY)?,
                })
            })();
            self.formats = built.ok();
        }
        let dc = self.dc_target.as_ref().unwrap();
        unsafe {
            if dc.BindDC(hdc, rc).is_err() {
                return false;
            }
            dc.BeginDraw();
        }
        self.brush.is_some() && self.formats.is_some()
    }

    /// Ends the frame; a lost device is rebuilt on the next one.
    pub fn end(&mut self) {
        if let Some(t) = &self.hwnd_target
            && let Err(e) = unsafe { t.EndDraw(None, None) }
            && e.code() == D2DERR_RECREATE_TARGET
        {
            self.hwnd_target = None;
            self.brush = None;
            self.preview = None;
            self.preview_ambient = None;
            self.layers.clear();
        } else if let Some(dc) = &self.dc_target {
            let _ = unsafe { dc.EndDraw(None, None) };
        }
        self.layer_depth = 0;
    }

    fn t(&self) -> &ID2D1RenderTarget {
        if let Some(h) = &self.hwnd_target {
            h
        } else {
            self.dc_target.as_ref().unwrap()
        }
    }

    fn brush(&self, rgb: u32) -> &ID2D1SolidColorBrush {
        self.brush_alpha(rgb, 1.0)
    }

    fn brush_alpha(&self, rgb: u32, alpha: f32) -> &ID2D1SolidColorBrush {
        let b = self.brush.as_ref().unwrap();
        unsafe { b.SetColor(&color_alpha(rgb, alpha)) };
        b
    }

    pub fn set_offset(&self, dx: f32, dy: f32) {
        let s = self.scale;
        let m = Matrix3x2::translation(dx * s, dy * s);
        unsafe { self.t().SetTransform(&m) };
    }

    pub fn reset_transform(&self) {
        unsafe { self.t().SetTransform(&Matrix3x2::identity()) };
    }

    pub fn push_layer(&mut self, r: Rect, opacity: f32) -> bool {
        if opacity >= 0.999 {
            return false;
        }
        if self.layer_depth >= self.layers.len() {
            let Ok(layer) = (unsafe { self.t().CreateLayer(None) }) else {
                return false;
            };
            self.layers.push(layer);
        }
        let layer = &self.layers[self.layer_depth];
        self.layer_depth += 1;
        let params = D2D1_LAYER_PARAMETERS {
            contentBounds: self.px(r),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: opacity.clamp(0.0, 1.0),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
            ..Default::default()
        };
        unsafe { self.t().PushLayer(&params, layer) };
        true
    }

    pub fn pop_layer(&mut self, pushed: bool) {
        if pushed {
            unsafe { self.t().PopLayer() };
            self.layer_depth = self.layer_depth.saturating_sub(1);
        }
    }

    pub fn push_rounded_clip(&mut self, r: Rect, radius: f32) -> bool {
        let s = self.scale;
        let rr = D2D1_ROUNDED_RECT {
            rect: self.px(r),
            radiusX: radius * s,
            radiusY: radius * s,
        };
        let Ok(geom) = (unsafe { self.d2d.CreateRoundedRectangleGeometry(&rr) }) else {
            return false;
        };
        if self.layer_depth >= self.layers.len() {
            let Ok(layer) = (unsafe { self.t().CreateLayer(None) }) else {
                return false;
            };
            self.layers.push(layer);
        }
        let layer = &self.layers[self.layer_depth];
        self.layer_depth += 1;
        let mut params = D2D1_LAYER_PARAMETERS {
            contentBounds: self.px(r),
            geometricMask: std::mem::ManuallyDrop::new(Some(geom.into())),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
            ..Default::default()
        };
        unsafe {
            self.t().PushLayer(&params, layer);
            let _ = std::mem::ManuallyDrop::take(&mut params.geometricMask);
        }
        true
    }

    /// A layout rectangle (100% scale) in physical pixels, its right and bottom edges placed so
    /// the gap to the next tile is exactly one pixel.
    pub fn px(&self, r: Rect) -> D2D_RECT_F {
        let s = self.scale;
        D2D_RECT_F {
            left: (r.x * s).round(),
            top: (r.y * s).round(),
            right: ((r.right() + 1.0) * s).round() - 1.0,
            bottom: ((r.bottom() + 1.0) * s).round() - 1.0,
        }
    }

    pub fn clear(&self, rgb: u32) {
        unsafe { self.t().Clear(Some(&color(rgb))) };
    }

    pub fn fill(&self, r: Rect, rgb: u32) {
        unsafe { self.t().FillRectangle(&self.px(r), self.brush(rgb)) };
    }

    pub fn fill_rounded(&self, r: Rect, radius: f32, rgb: u32) {
        self.fill_rounded_alpha(r, radius, rgb, 1.0);
    }

    pub fn fill_rounded_alpha(&self, r: Rect, radius: f32, rgb: u32, alpha: f32) {
        let s = self.scale;
        let rr = D2D1_ROUNDED_RECT {
            rect: self.px(r),
            radiusX: radius * s,
            radiusY: radius * s,
        };
        unsafe {
            self.t()
                .FillRoundedRectangle(&rr, self.brush_alpha(rgb, alpha))
        };
    }

    pub fn stroke_rounded(&self, r: Rect, radius: f32, rgb: u32, width: f32) {
        self.stroke_rounded_alpha(r, radius, rgb, 1.0, width);
    }

    pub fn stroke_rounded_alpha(&self, r: Rect, radius: f32, rgb: u32, alpha: f32, width: f32) {
        let s = self.scale;
        let rr = D2D1_ROUNDED_RECT {
            rect: self.px(r),
            radiusX: radius * s,
            radiusY: radius * s,
        };
        unsafe {
            self.t().DrawRoundedRectangle(
                &rr,
                self.brush_alpha(rgb, alpha),
                width * s,
                &self.stroke,
            )
        };
    }

    pub fn circle(&self, cx: f32, cy: f32, r: f32, rgb: u32) {
        let s = self.scale;
        let e = D2D1_ELLIPSE {
            point: Vector2 {
                X: cx * s,
                Y: cy * s,
            },
            radiusX: r * s,
            radiusY: r * s,
        };
        unsafe { self.t().FillEllipse(&e, self.brush(rgb)) };
    }

    #[allow(dead_code)]
    pub fn stroke_circle(&self, cx: f32, cy: f32, r: f32, rgb: u32, width: f32) {
        let s = self.scale;
        let e = D2D1_ELLIPSE {
            point: Vector2 {
                X: cx * s,
                Y: cy * s,
            },
            radiusX: r * s,
            radiusY: r * s,
        };
        unsafe {
            self.t()
                .DrawEllipse(&e, self.brush(rgb), width * s, &self.stroke)
        };
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, rgb: u32, width: f32) {
        self.line_alpha((x1, y1), (x2, y2), rgb, 1.0, width);
    }

    pub fn line_alpha(&self, from: (f32, f32), to: (f32, f32), rgb: u32, alpha: f32, width: f32) {
        let s = self.scale;
        let p1 = Vector2 {
            X: (from.0 * s).round(),
            Y: (from.1 * s).round(),
        };
        let p2 = Vector2 {
            X: (to.0 * s).round(),
            Y: (to.1 * s).round(),
        };
        unsafe {
            self.t().DrawLine(
                p1,
                p2,
                self.brush_alpha(rgb, alpha),
                width * s,
                &self.stroke,
            );
        }
    }

    pub fn dots_grid(&self, r: Rect, spacing: f32, dot_r: f32, rgb: u32) {
        let mut y = r.y + spacing / 2.0;
        while y < r.bottom() {
            let mut x = r.x + spacing / 2.0;
            while x < r.right() {
                self.circle(x, y, dot_r, rgb);
                x += spacing;
            }
            y += spacing;
        }
    }

    #[allow(dead_code)]
    pub fn fader(&self, r: Rect, progress: f32, rgb_fill: u32, rgb_track: u32) {
        let track_h = 7.0;
        let track_r = Rect::new(r.x, r.y + (r.h - track_h) / 2.0, r.w, track_h);
        self.fill_rounded(track_r, 3.5, rgb_track);
        let fill_w = (r.w * progress.clamp(0.0, 1.0)).max(0.0);
        if fill_w > 0.0 {
            self.fill_rounded(Rect::new(r.x, track_r.y, fill_w, track_h), 3.5, rgb_fill);
        }
        let cap_x = r.x + fill_w;
        let cap_w = 2.5;
        let cap_h = 13.0;
        let cap_r = Rect::new(cap_x - cap_w / 2.0, r.y + (r.h - cap_h) / 2.0, cap_w, cap_h);
        self.fill_rounded(cap_r, 1.25, 0xFFFFFF);
    }

    #[allow(dead_code)]
    pub fn switch(&self, r: Rect, on: bool) {
        if on {
            self.fill_rounded(r, r.h / 2.0, 0xFFFFFF);
            let knob_r = (r.h - 4.0) / 2.0;
            let knob_cx = r.right() - 2.0 - knob_r;
            let knob_cy = r.y + r.h / 2.0;
            self.circle(knob_cx, knob_cy, knob_r, 0x000000);
        } else {
            self.fill_rounded(r, r.h / 2.0, 0x000000);
            self.stroke_rounded(r, r.h / 2.0, 0x3A3A3C, 1.0);
            let knob_r = (r.h - 6.0) / 2.0;
            let knob_cx = r.x + 3.0 + knob_r;
            let knob_cy = r.y + r.h / 2.0;
            self.circle(knob_cx, knob_cy, knob_r, 0x3A3A3C);
        }
    }

    #[allow(dead_code)]
    pub fn waveform_bars(&self, r: Rect, bars: usize, active: bool, anim_seed: u64) {
        if bars == 0 || r.w <= 0.0 {
            return;
        }
        let bar_w = 3.0;
        let gap = ((r.w - (bars as f32 * bar_w)) / (bars as f32 - 1.0).max(1.0)).max(1.0);
        let start_x = r.x + (r.w - (bars as f32 * bar_w + (bars as f32 - 1.0) * gap)) / 2.0;
        for i in 0..bars {
            let bx = start_x + i as f32 * (bar_w + gap);
            let h = if active {
                let wave = ((i as f32 * 0.45 + (anim_seed as f32 * 0.18)).sin() * 0.5 + 0.5)
                    * ((i as f32 * 0.25 - (anim_seed as f32 * 0.12)).cos() * 0.5 + 0.5);
                let min_h = 4.0;
                let max_h = r.h;
                min_h + (max_h - min_h) * wave
            } else {
                3.0
            };
            let by = r.y + (r.h - h) / 2.0;
            self.fill_rounded(
                Rect::new(bx, by, bar_w, h),
                bar_w / 2.0,
                if active { 0xFFFFFF } else { 0x3A3A3C },
            );
        }
    }

    pub fn watermark(&self, cx: f32, cy: f32, size: f32, opacity: f32) {
        self.fill_brand_owl(cx, cy, size, opacity);
    }

    pub fn fill_brand_owl(&self, cx: f32, cy: f32, size: f32, opacity: f32) {
        let s = self.scale;
        let brush = self.brush(0xFFFFFF);
        unsafe {
            brush.SetOpacity(opacity);
            draw_brand_owl(&self.d2d, self.t(), brush, cx * s, cy * s, size * s);
            brush.SetOpacity(1.0);
        }
    }

    pub fn clip(&self, r: Rect) {
        unsafe {
            self.t()
                .PushAxisAlignedClip(&self.px(r), D2D1_ANTIALIAS_MODE_ALIASED)
        };
    }

    pub fn unclip(&self) {
        unsafe { self.t().PopAxisAlignedClip() };
    }

    /// Text in `r`, vertically centred. Long text wraps when `wrap`, else ends in an ellipsis.
    pub fn text(&self, text: &str, r: Rect, font: Font, rgb: u32, align: Align, wrap: bool) {
        let f = self.formats.as_ref().unwrap();
        let format = match font {
            Font::Title => &f.title,
            Font::Body => &f.body,
            Font::Caption => &f.caption,
            Font::Code => &f.code,
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let _ = format.SetTextAlignment(match align {
                Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            });
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = format.SetWordWrapping(if wrap {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            });
            let trimming = DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            };
            if let Ok(sign) = self.dwrite.CreateEllipsisTrimmingSign(format) {
                let _ = format.SetTrimming(&trimming, &sign);
            }
            self.t().DrawText(
                &wide,
                format,
                &self.px(r),
                self.brush(rgb),
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// A Lucide icon `size` px wide (at 100%) centred on (cx, cy), stroked in `rgb`.
    pub fn icon(&self, icon: Icon, cx: f32, cy: f32, size: f32, rgb: u32) {
        self.icon_impl(icon, cx, cy, size, rgb, false);
    }

    /// A Lucide icon rotated 180° (e.g. `arrow.svg` used as a Back button, matching prototype `rotate-180`).
    pub fn icon_rotated_180(&self, icon: Icon, cx: f32, cy: f32, size: f32, rgb: u32) {
        self.icon_impl(icon, cx, cy, size, rgb, true);
    }

    /// A Lucide icon rotated by arbitrary clockwise `angle_deg` around `(cx, cy)`.
    pub fn icon_rotated(&self, icon: Icon, cx: f32, cy: f32, size: f32, rgb: u32, angle_deg: f32) {
        if angle_deg.abs() < 0.01 {
            self.icon_impl(icon, cx, cy, size, rgb, false);
            return;
        }
        let s = self.scale;
        let rad = angle_deg.to_radians();
        let (sin, cos) = rad.sin_cos();
        let (pcx, pcy) = (cx * s, cy * s);
        let rot = Matrix3x2 {
            M11: cos,
            M12: sin,
            M21: -sin,
            M22: cos,
            M31: pcx * (1.0 - cos) + pcy * sin,
            M32: pcy * (1.0 - cos) - pcx * sin,
        };
        let mut prev = Matrix3x2::identity();
        unsafe {
            self.t().GetTransform(&mut prev);
            let combined = Matrix3x2 {
                M11: rot.M11 * prev.M11 + rot.M12 * prev.M21,
                M12: rot.M11 * prev.M12 + rot.M12 * prev.M22,
                M21: rot.M21 * prev.M11 + rot.M22 * prev.M21,
                M22: rot.M21 * prev.M12 + rot.M22 * prev.M22,
                M31: rot.M31 * prev.M11 + rot.M32 * prev.M21 + prev.M31,
                M32: rot.M31 * prev.M12 + rot.M32 * prev.M22 + prev.M32,
            };
            self.t().SetTransform(&combined);
            self.icon_impl(icon, cx, cy, size, rgb, false);
            self.t().SetTransform(&prev);
        }
    }

    fn icon_impl(&self, icon: Icon, cx: f32, cy: f32, size: f32, rgb: u32, rot_180: bool) {
        let s = self.scale;
        let k = size * s / BOX;
        let (ox, oy) = (cx * s - size * s / 2.0, cy * s - size * s / 2.0);
        let p = move |x: f32, y: f32| {
            let (tx, ty) = if rot_180 { (BOX - x, BOX - y) } else { (x, y) };
            Vector2 {
                X: ox + tx * k,
                Y: oy + ty * k,
            }
        };
        let width = crate::icon::STROKE * k;
        let brush = self.brush(rgb);
        for shape in icon {
            unsafe {
                match *shape {
                    Shape::Circle { c, r } => {
                        let e = D2D1_ELLIPSE {
                            point: p(c.0, c.1),
                            radiusX: r * k,
                            radiusY: r * k,
                        };
                        self.t().DrawEllipse(&e, brush, width, &self.stroke);
                    }
                    Shape::Rect { x, y, w, h, r } => {
                        let (rx, ry) = if rot_180 {
                            (BOX - x - w, BOX - y - h)
                        } else {
                            (x, y)
                        };
                        let rr = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: ox + rx * k,
                                top: oy + ry * k,
                                right: ox + (rx + w) * k,
                                bottom: oy + (ry + h) * k,
                            },
                            radiusX: r * k,
                            radiusY: r * k,
                        };
                        self.t()
                            .DrawRoundedRectangle(&rr, brush, width, &self.stroke);
                    }
                    Shape::Path(segs) => {
                        if let Ok(geometry) = self.path(segs, &p, k) {
                            self.t().DrawGeometry(&geometry, brush, width, &self.stroke);
                        }
                    }
                }
            }
        }
    }

    fn path(
        &self,
        segs: &[Seg],
        p: &dyn Fn(f32, f32) -> Vector2,
        k: f32,
    ) -> Result<ID2D1PathGeometry> {
        unsafe {
            let geometry = self.d2d.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            let mut open = false;
            for seg in segs {
                match *seg {
                    Seg::Move(x, y) => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        sink.BeginFigure(p(x, y), D2D1_FIGURE_BEGIN_HOLLOW);
                        open = true;
                    }
                    Seg::Line(x, y) => sink.AddLine(p(x, y)),
                    Seg::Cubic(c) => sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                        point1: p(c[0], c[1]),
                        point2: p(c[2], c[3]),
                        point3: p(c[4], c[5]),
                    }),
                    Seg::Arc {
                        radius,
                        rotation,
                        large,
                        clockwise,
                        to,
                    } => sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: p(to.0, to.1),
                        size: D2D_SIZE_F {
                            width: radius.0 * k,
                            height: radius.1 * k,
                        },
                        rotationAngle: rotation,
                        sweepDirection: if clockwise {
                            D2D1_SWEEP_DIRECTION_CLOCKWISE
                        } else {
                            D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE
                        },
                        arcSize: if large {
                            D2D1_ARC_SIZE_LARGE
                        } else {
                            D2D1_ARC_SIZE_SMALL
                        },
                    }),
                    Seg::Close => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            open = false;
                        }
                    }
                }
            }
            if open {
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            sink.Close()?;
            Ok(geometry)
        }
    }

    /// The camera preview (BGRA `w` x `h`), refreshed when `seq` changed.
    /// When `ambient` is `Some`, draws a blurred zoomed dimmed backdrop inside `ambient` first.
    pub fn picture(
        &mut self,
        r: Rect,
        ambient: Option<Rect>,
        w: usize,
        h: usize,
        seq: u64,
        pixels: impl FnOnce(&mut dyn FnMut(&[u8])),
    ) {
        const AMB: usize = 36;
        if w == 0 || h == 0 {
            return;
        }
        let size = D2D_SIZE_U {
            width: w as u32,
            height: h as u32,
        };
        let dest = self.px(r);
        let amb_dest = ambient.map(|a| self.px(a));
        if let Some((b, _)) = &self.preview
            && unsafe { b.GetPixelSize() } != size
        {
            self.preview = None;
        }
        let props = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        if self.preview.is_none() {
            self.preview =
                unsafe { self.t().CreateBitmap(size, None, 0, &props).ok() }.map(|b| (b, u64::MAX));
        }
        if ambient.is_some() && self.preview_ambient.is_none() {
            let amb_size = D2D_SIZE_U {
                width: AMB as u32,
                height: AMB as u32,
            };
            self.preview_ambient = unsafe { self.t().CreateBitmap(amb_size, None, 0, &props).ok() }
                .map(|b| (b, u64::MAX));
        }
        let need_amb = ambient.is_some()
            && self
                .preview_ambient
                .as_ref()
                .is_some_and(|(_, shown)| *shown != seq);
        let need_main = self
            .preview
            .as_ref()
            .is_some_and(|(_, shown)| *shown != seq);
        if need_main || need_amb {
            let mut amb_buf = need_amb.then(|| vec![0u8; AMB * AMB * 4]);
            pixels(&mut |px: &[u8]| {
                if px.len() < (w * h * 4) {
                    return;
                }
                if need_main && let Some((bitmap, _)) = self.preview.as_ref() {
                    let len = (w * 4) as u32;
                    unsafe {
                        let _ = bitmap.CopyFromMemory(None, px.as_ptr().cast(), len);
                    }
                }
                if let Some(out) = amb_buf.as_mut() {
                    build_ambient_bgra::<AMB>(px, w, h, out);
                    if let Some((amb_bmp, _)) = self.preview_ambient.as_ref() {
                        unsafe {
                            let _ =
                                amb_bmp.CopyFromMemory(None, out.as_ptr().cast(), (AMB * 4) as u32);
                        }
                    }
                }
            });
            if need_main && let Some((_, shown)) = self.preview.as_mut() {
                *shown = seq;
            }
            if need_amb && let Some((_, shown)) = self.preview_ambient.as_mut() {
                *shown = seq;
            }
        }
        if let Some(amb_rect) = amb_dest
            && let Some((amb_bmp, _)) = &self.preview_ambient
        {
            let amb_bmp = amb_bmp.clone();
            unsafe {
                self.t().DrawBitmap(
                    &amb_bmp,
                    Some(&amb_rect),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        }
        let Some((bitmap, _)) = self.preview.as_ref() else {
            return;
        };
        let bitmap = bitmap.clone();
        let src_ar = w as f32 / h as f32;
        let dst_ar = r.w / r.h.max(0.001);
        let src_rect = if (src_ar - dst_ar).abs() < 0.005 {
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: w as f32,
                bottom: h as f32,
            }
        } else if src_ar > dst_ar {
            let sw = (h as f32 * dst_ar).min(w as f32);
            let sx = (w as f32 - sw) / 2.0;
            D2D_RECT_F {
                left: sx,
                top: 0.0,
                right: sx + sw,
                bottom: h as f32,
            }
        } else {
            let sh = (w as f32 / dst_ar).min(h as f32);
            let sy = (h as f32 - sh) / 2.0;
            D2D_RECT_F {
                left: 0.0,
                top: sy,
                right: w as f32,
                bottom: sy + sh,
            }
        };
        unsafe {
            self.t().DrawBitmap(
                &bitmap,
                Some(&dest),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                Some(&src_rect),
            )
        };
    }
}

/// Downsamples `px` into a blurred, slightly zoomed (1.14x), dimmed (0.46) and boosted-saturation
/// (1.18) `N x N` BGRA backdrop matching prototype `.cam-amb-canvas`.
fn build_ambient_bgra<const N: usize>(px: &[u8], w: usize, h: usize, out: &mut [u8]) {
    let mut a = vec![[0.0f32; 3]; N * N];
    let mut b = vec![[0.0f32; 3]; N * N];
    let span = 1.0 / 1.14_f32;
    let pad = (1.0 - span) * 0.5;
    for y in 0..N {
        let sy = (((pad + (y as f32 + 0.5) / N as f32 * span) * h as f32) as usize).min(h - 1);
        for x in 0..N {
            let sx = (((pad + (x as f32 + 0.5) / N as f32 * span) * w as f32) as usize).min(w - 1);
            let idx = (sy * w + sx) * 4;
            a[y * N + x] = [px[idx] as f32, px[idx + 1] as f32, px[idx + 2] as f32];
        }
    }
    const R: isize = 4;
    for pass in 0..2 {
        for y in 0..N {
            for x in 0..N {
                let mut sum = [0.0f32; 3];
                let mut cnt = 0.0f32;
                for d in -R..=R {
                    let nx = (x as isize + d).clamp(0, N as isize - 1) as usize;
                    let c = a[y * N + nx];
                    sum[0] += c[0];
                    sum[1] += c[1];
                    sum[2] += c[2];
                    cnt += 1.0;
                }
                b[y * N + x] = [sum[0] / cnt, sum[1] / cnt, sum[2] / cnt];
            }
        }
        for y in 0..N {
            for x in 0..N {
                let mut sum = [0.0f32; 3];
                let mut cnt = 0.0f32;
                for d in -R..=R {
                    let ny = (y as isize + d).clamp(0, N as isize - 1) as usize;
                    let c = b[ny * N + x];
                    sum[0] += c[0];
                    sum[1] += c[1];
                    sum[2] += c[2];
                    cnt += 1.0;
                }
                a[y * N + x] = [sum[0] / cnt, sum[1] / cnt, sum[2] / cnt];
            }
        }
        let _ = pass;
    }
    for (i, c) in a.iter().enumerate() {
        let lum = 0.114 * c[0] + 0.587 * c[1] + 0.299 * c[2];
        let o = i * 4;
        for ch in 0..3 {
            out[o + ch] = ((lum + (c[ch] - lum) * 1.18) * 0.46).clamp(0.0, 255.0) as u8;
        }
        out[o + 3] = 255;
    }
}

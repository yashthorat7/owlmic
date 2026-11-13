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
use windows_numerics::Vector2;

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
    Code,
}

fn color(rgb: u32) -> D2D1_COLOR_F {
    let c = |shift: u32| ((rgb >> shift) & 0xFF) as f32 / 255.0;
    D2D1_COLOR_F {
        r: c(16),
        g: c(8),
        b: c(0),
        a: 1.0,
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
    target: Option<ID2D1HwndRenderTarget>,
    brush: Option<ID2D1SolidColorBrush>,
    formats: Option<Formats>,
    family: HSTRING,
    mono: HSTRING,
    preview: Option<(ID2D1Bitmap, u64)>,
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
                target: None,
                brush: None,
                formats: None,
                family,
                mono,
                preview: None,
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
            f.SetLineSpacing(
                DWRITE_LINE_SPACING_METHOD_UNIFORM,
                style.line_px * self.scale,
                style.size_px * self.scale * 0.8,
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
        if self.target.is_none() {
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
            self.target = Some(target);
            self.preview = None;
        }
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
        let target = self.target.as_ref().unwrap();
        unsafe {
            let _ = target.Resize(&size);
            target.BeginDraw();
        }
        self.brush.is_some() && self.formats.is_some()
    }

    /// Ends the frame; a lost device is rebuilt on the next one.
    pub fn end(&mut self) {
        if let Some(t) = &self.target
            && let Err(e) = unsafe { t.EndDraw(None, None) }
            && e.code() == D2DERR_RECREATE_TARGET
        {
            self.target = None;
            self.preview = None;
        }
    }

    fn t(&self) -> &ID2D1HwndRenderTarget {
        self.target.as_ref().unwrap()
    }

    fn brush(&self, rgb: u32) -> &ID2D1SolidColorBrush {
        let b = self.brush.as_ref().unwrap();
        unsafe { b.SetColor(&color(rgb)) };
        b
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
        let s = self.scale;
        let k = size * s / BOX;
        let (ox, oy) = (cx * s - size * s / 2.0, cy * s - size * s / 2.0);
        let p = |x: f32, y: f32| Vector2 {
            X: ox + x * k,
            Y: oy + y * k,
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
                        let rr = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: ox + x * k,
                                top: oy + y * k,
                                right: ox + (x + w) * k,
                                bottom: oy + (y + h) * k,
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
    pub fn picture(
        &mut self,
        r: Rect,
        w: usize,
        h: usize,
        seq: u64,
        pixels: impl FnOnce(&mut dyn FnMut(&[u8])),
    ) {
        let size = D2D_SIZE_U {
            width: w as u32,
            height: h as u32,
        };
        let dest = self.px(r);
        if self.preview.is_none() {
            let props = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            self.preview =
                unsafe { self.t().CreateBitmap(size, None, 0, &props).ok() }.map(|b| (b, u64::MAX));
        }
        let Some((bitmap, shown)) = self.preview.as_mut() else {
            return;
        };
        if *shown != seq {
            pixels(&mut |px: &[u8]| unsafe {
                let _ = bitmap.CopyFromMemory(None, px.as_ptr().cast(), (w * 4) as u32);
            });
            *shown = seq;
        }
        let bitmap = bitmap.clone();
        unsafe {
            self.t().DrawBitmap(
                &bitmap,
                Some(&dest),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            )
        };
    }
}

//! The picture Owlmic Cam shows with no phone: black, with the centered Owlmic owl symbol in
//! the secondary text colour. Drawn once with Direct2D.

use owlmic_media::video::Nv12;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES, D2D1CreateFactory,
    ID2D1Factory,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, SelectObject,
};

/// Renders the centered Owlmic owl placeholder frame in NV12.
pub fn render(width: usize, height: usize) -> Nv12 {
    unsafe {
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            return Nv12::black(width, height);
        };
        let old_bitmap = SelectObject(dc, bitmap.into());

        if let Ok(d2d) = D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
        {
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            if let Ok(target) = d2d.CreateDCRenderTarget(&props) {
                let rc = RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                if target.BindDC(dc, &rc).is_ok() {
                    let rgb = owlmic_ui::color::TEXT2;
                    let owl_color = D2D1_COLOR_F {
                        r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
                        g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
                        b: (rgb & 0xFF) as f32 / 255.0,
                        a: 1.0,
                    };
                    target.BeginDraw();
                    target.Clear(Some(&D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }));
                    if let Ok(brush) = target.CreateSolidColorBrush(&owl_color, None) {
                        let size = height as f32 / 6.0;
                        owlmic_ui::win::draw_brand_owl(
                            &d2d,
                            &target,
                            &brush,
                            width as f32 / 2.0,
                            height as f32 / 2.0,
                            size,
                        );
                    }
                    let _ = target.EndDraw(None, None);
                }
            }
        }

        let _ = GdiFlush();
        let pixels = std::slice::from_raw_parts(bits as *const u8, width * height * 4);
        let frame = Nv12::from_bgra(width, height, pixels);
        SelectObject(dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_draws_owl_on_black_background() {
        let frame = render(192, 108);
        let y_plane = &frame.data[..192 * 108];
        let min_y = *y_plane.iter().min().unwrap();
        let max_y = *y_plane.iter().max().unwrap();
        assert!(min_y <= 20, "background should be black, min luma {min_y}");
        assert!(
            max_y >= 100,
            "owl symbol should be visible, max luma {max_y}"
        );
    }
}

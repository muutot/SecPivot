//! Screen capture and barcode (QR / DataMatrix / Aztec / PDF417 / 1D)
//! decoding for the TOTP seed picker. The screen is captured as PNG via GDI
//! and decoded in-process with rxing; results are plain strings returned to
//! the renderer, nothing is persisted or logged.

use std::io::Cursor;

/// Capture the entire virtual desktop (all monitors) and return it as a PNG.
#[cfg(windows)]
#[tauri::command]
pub fn capture_screen_png() -> Result<Vec<u8>, String> {
    let (rgba, width, height) = capture_virtual_screen()?;
    encode_png(&rgba, width, height)
}

/// Non-Windows desktop targets have no capture implementation yet.
#[cfg(not(windows))]
#[tauri::command]
pub fn capture_screen_png() -> Result<Vec<u8>, String> {
    Err("当前平台暂不支持屏幕截图".to_owned())
}

/// Decode every barcode found in a PNG image (full screenshot or user-drawn
/// region). Returns all hits so the UI can disambiguate multiple codes.
#[tauri::command]
pub fn decode_barcode_png(png: Vec<u8>) -> Result<Vec<String>, String> {
    let img = image::load_from_memory(&png).map_err(|e| format!("图片解码失败: {e}"))?;
    let hits = match rxing::helpers::detect_multiple_in_image(img) {
        Ok(hits) => hits,
        // A clean image with no codes is a normal outcome, not an error.
        Err(rxing::Exceptions::NotFoundException(_)) => Vec::new(),
        Err(e) => return Err(format!("识别失败: {e}")),
    };
    Ok(hits.iter().map(|r| r.getText().to_owned()).collect())
}

#[cfg(windows)]
fn capture_virtual_screen() -> Result<(Vec<u8>, u32, u32), String> {
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
        DIB_RGB_COLORS, RGBQUAD, SRCCOPY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if w <= 0 || h <= 0 {
            return Err("无法获取屏幕尺寸".to_owned());
        }
        let (w, h) = (w as u32, h as u32);

        let hdc_screen = GetDC(std::ptr::null_mut());
        if hdc_screen.is_null() {
            return Err("无法获取屏幕设备上下文".to_owned());
        }
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbmp = CreateCompatibleBitmap(hdc_screen, w as i32, h as i32);
        let old = SelectObject(hdc_mem, hbmp);

        let ok = BitBlt(
            hdc_mem,
            0,
            0,
            w as i32,
            h as i32,
            hdc_screen,
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        );

        let mut rgba = Vec::new();
        if ok != 0 {
            // Top-down 32bpp BGR buffer; alpha byte is unused padding.
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };
            rgba = vec![0u8; (w * h * 4) as usize];
            let copied = GetDIBits(
                hdc_mem,
                hbmp,
                0,
                h,
                rgba.as_mut_ptr().cast(),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            if copied == 0 {
                rgba.clear();
            } else {
                for px in rgba.chunks_exact_mut(4) {
                    px.swap(0, 2); // BGRA -> RGBA
                }
            }
        }

        SelectObject(hdc_mem, old);
        DeleteObject(hbmp);
        DeleteDC(hdc_mem);
        ReleaseDC(std::ptr::null_mut(), hdc_screen);

        if rgba.is_empty() {
            return Err("屏幕截图失败".to_owned());
        }
        Ok((rgba, w, h))
    }
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let img =
        image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or("截图数据尺寸不匹配")?;
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rxing::{BarcodeFormat, MultiFormatWriter, Writer};

    #[test]
    fn decodes_generated_qr_png() {
        let payload = "otpauth://totp/SecPivot:test?secret=JBSWY3DPEHPK3PXP&issuer=SecPivot";
        let matrix = MultiFormatWriter
            .encode(payload, &BarcodeFormat::QR_CODE, 240, 240)
            .expect("qr encode");
        let mut img = image::GrayImage::new(matrix.getWidth(), matrix.getHeight());
        for y in 0..matrix.getHeight() {
            for x in 0..matrix.getWidth() {
                let v = if matrix.get(x, y) { 0u8 } else { 255 };
                img.put_pixel(x, y, image::Luma([v]));
            }
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png encode");

        assert_eq!(decode_barcode_png(png).unwrap(), vec![payload.to_owned()]);
    }

    #[test]
    fn reports_missing_codes_as_empty() {
        // 1x1 white PNG: no barcode present.
        let img = image::GrayImage::from_pixel(1, 1, image::Luma([255]));
        let mut png = Vec::new();
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png encode");
        assert!(decode_barcode_png(png).unwrap().is_empty());
    }
}

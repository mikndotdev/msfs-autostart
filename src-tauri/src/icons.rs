//! Program icons for the UI: asks the Windows shell for a file's icon (works for
//! .exe, .lnk and .bat the same way Explorer does) and returns it as a PNG data URL.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::Mutex;

/// Successful lookups only, so a program installed later still gets its icon.
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn icon_data_url(path: &str) -> Option<String> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() || !Path::new(path).exists() {
        return None;
    }
    let key = path.to_lowercase();
    if let Some(hit) = CACHE.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Some(hit.clone());
    }
    let (w, h, rgba) = extract_rgba(path)?;
    let url = format!("data:image/png;base64,{}", STANDARD.encode(encode_png(w, h, &rgba)?));
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, url.clone());
    Some(url)
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// 32x32 icon as RGBA pixels.
fn extract_rgba(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        // The shell needs COM on the calling thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let out = hicon_to_rgba(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        out
    }
}

unsafe fn hicon_to_rgba(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HBITMAP, HDC,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    unsafe fn read_bits(hdc: HDC, bmp: HBITMAP, w: i32, h: i32) -> Option<Vec<u8>> {
        unsafe {
            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(hdc, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut c_void), &mut bi, DIB_RGB_COLORS);
            (lines == h).then_some(buf)
        }
    }

    unsafe {
        let mut ii = ICONINFO::default();
        GetIconInfo(icon, &mut ii).ok()?;
        let (color, mask) = (ii.hbmColor, ii.hbmMask);
        let cleanup = || {
            if !color.is_invalid() {
                let _ = DeleteObject(color.into());
            }
            if !mask.is_invalid() {
                let _ = DeleteObject(mask.into());
            }
        };
        if color.is_invalid() {
            // Monochrome icon; not worth supporting.
            cleanup();
            return None;
        }
        let mut bm = BITMAP::default();
        if GetObjectW(color.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut c_void)) == 0 {
            cleanup();
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        let hdc = GetDC(None);
        let pixels = read_bits(hdc, color, w, h);
        // Old icons have no alpha channel; their transparency is in the AND mask.
        let mask_bits = match &pixels {
            Some(p) if !p.chunks_exact(4).any(|px| px[3] != 0) => read_bits(hdc, mask, w, h),
            _ => None,
        };
        ReleaseDC(None, hdc);
        cleanup();

        let mut rgba = pixels?;
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            px.swap(0, 2); // BGRA -> RGBA
            if let Some(m) = &mask_bits {
                // Mask pixel white = transparent.
                px[3] = if m[i * 4] == 0 { 255 } else { 0 };
            }
        }
        Some((w as u32, h as u32, rgba))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo test real_icons -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_icons() {
        for p in [r"C:\Windows\explorer.exe", r"C:\Windows\System32\notepad.exe", r"C:\Program Files\Notepad++\notepad++.exe"] {
            let url = icon_data_url(p);
            println!("{p}: {}", url.as_deref().map_or("none".to_string(), |u| format!("{} bytes", u.len())));
            if Path::new(p).exists() {
                assert!(url.is_some(), "no icon for {p}");
            }
        }
        assert!(icon_data_url(r"C:\does\not\exist.exe").is_none());
        // Write one out for eyeballing: ICON_OUT=path\to\file.png
        if let (Ok(out), Some(url)) = (std::env::var("ICON_OUT"), icon_data_url(r"C:\Program Files\Notepad++\notepad++.exe")) {
            let b64 = url.trim_start_matches("data:image/png;base64,");
            std::fs::write(out, STANDARD.decode(b64).unwrap()).unwrap();
        }
    }
}

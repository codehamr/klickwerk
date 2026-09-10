use super::{default_desktop, now};
use crate::{action::Action, guard::Frame};
use image::{RgbImage, imageops::FilterType};
use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};

pub struct Capture {
    pub frame: Frame,
    pub pixels: RgbImage,
    pub jpeg: Vec<u8>,
}

pub fn layout() -> (i32, i32, u32, u32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN) as u32,
            GetSystemMetrics(SM_CYVIRTUALSCREEN) as u32,
        )
    }
}

pub fn capture(id: u64, max_edge: u32, excluded: [i32; 4]) -> Result<Capture, String> {
    if !default_desktop() {
        return Err("Control stopped because the active desktop changed.".into());
    }
    let (left, top, width, height) = layout();
    if width == 0 || height == 0 || width as u64 * height as u64 > 32_000_000 {
        return Err("This desktop layout exceeds the supported capture size.".into());
    }
    let foreground = unsafe { GetForegroundWindow() } as usize;
    let captured_ms = now();
    let mut pixels = read_pixels(left, top, width, height)?;
    if pixels
        .as_raw()
        .as_chunks::<3>()
        .0
        .iter()
        .all(|p| p[0] < 3 && p[1] < 3 && p[2] < 3)
    {
        return Err(
            "The desktop could not be captured. Protected or locked desktops cannot be controlled."
                .into(),
        );
    }
    for y in excluded[1].max(top)..excluded[3].min(top + height as i32) {
        for x in excluded[0].max(left)..excluded[2].min(left + width as i32) {
            pixels.put_pixel(
                (x - left) as u32,
                (y - top) as u32,
                image::Rgb([30, 30, 36]),
            );
        }
    }
    let scale = (max_edge as f64 / width.max(height) as f64).min(1.0);
    let image_width = (width as f64 * scale).round().max(1.0) as u32;
    let image_height = (height as f64 * scale).round().max(1.0) as u32;
    let resized = image::imageops::resize(&pixels, image_width, image_height, FilterType::Triangle);
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
        .encode_image(&resized)
        .map_err(|_| "The screenshot could not be encoded.")?;
    if jpeg.len() > 8 * 1024 * 1024 {
        return Err("The screenshot exceeds 8 MiB.".into());
    }
    Ok(Capture {
        frame: Frame {
            id,
            captured_ms,
            left,
            top,
            width,
            height,
            image_width,
            image_height,
            foreground,
        },
        pixels,
        jpeg,
    })
}

pub fn read_pixels(left: i32, top: i32, width: u32, height: u32) -> Result<RgbImage, String> {
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("The desktop is unavailable.".into());
        }
        let memory = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width as i32;
        info.bmiHeader.biHeight = -(height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            screen,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        if memory.is_null() || bitmap.is_null() || bits.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            if !memory.is_null() {
                DeleteDC(memory);
            }
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("The screenshot buffer could not be created.".into());
        }
        let old = SelectObject(memory, bitmap);
        let ok = BitBlt(
            memory,
            0,
            0,
            width as i32,
            height as i32,
            screen,
            left,
            top,
            SRCCOPY | CAPTUREBLT,
        );
        GdiFlush();
        let mut rgb = RgbImage::new(width, height);
        if ok != 0 {
            let raw = std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);
            for (dst, src) in rgb
                .as_mut()
                .as_chunks_mut::<3>()
                .0
                .iter_mut()
                .zip(raw.as_chunks::<4>().0.iter())
            {
                dst.copy_from_slice(&[src[2], src[1], src[0]]);
            }
        }
        SelectObject(memory, old);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 {
            Err("The desktop screenshot failed.".into())
        } else {
            Ok(rgb)
        }
    }
}

// Recheck a local target patch after the model response, before granting input.
pub fn unchanged(capture: &Capture, action: &Action) -> Result<(), String> {
    let frame = &capture.frame;
    if layout() != (frame.left, frame.top, frame.width, frame.height) {
        return Err("The monitor layout changed. Start the task again.".into());
    }
    let points = action.points();
    if points.is_empty() {
        if unsafe { GetForegroundWindow() } as usize != frame.foreground {
            return Err(
                "The focused window changed. Start again when the desktop is ready.".into(),
            );
        }
        return Ok(());
    }
    for (x, y) in points {
        let (px, py) = frame
            .map(x, y)
            .ok_or("The target is outside the screenshot.")?;
        let left = (px - 16).max(frame.left);
        let top = (py - 16).max(frame.top);
        let width = 32.min(frame.left + frame.width as i32 - left) as u32;
        let height = 32.min(frame.top + frame.height as i32 - top) as u32;
        let current = read_pixels(left, top, width, height)?;
        let mut different = 0;
        for y in 0..height {
            for x in 0..width {
                let old = capture
                    .pixels
                    .get_pixel((left - frame.left) as u32 + x, (top - frame.top) as u32 + y);
                let new = current.get_pixel(x, y);
                if old.0.iter().zip(new.0).any(|(&a, b)| a.abs_diff(b) > 18) {
                    different += 1;
                }
            }
        }
        if different > width * height / 12 {
            return Err(
                "The target moved while the model was thinking. Start again on a stable desktop."
                    .into(),
            );
        }
    }
    Ok(())
}

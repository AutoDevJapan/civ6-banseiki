//! Win32 window capture and portable PNG encoding.
use crate::{time::monotonic, Error, Result};
use flate2::{write::ZlibEncoder, Compression};
use std::{fs::OpenOptions, io::Write, path::Path};

fn check_deadline(deadline: Option<f64>) -> Result<()> {
    if deadline.is_some_and(|end| monotonic() >= end) {
        Err(Error::capture("Screenshot deadline expired"))
    } else {
        Ok(())
    }
}

pub fn crop_bgra(
    width: usize,
    height: usize,
    pixels: &[u8],
    crop: Option<[f64; 4]>,
) -> Result<(usize, usize, Vec<u8>)> {
    let len = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(|| Error::capture("Capture dimensions overflow"))?;
    if width == 0 || height == 0 || pixels.len() != len {
        return Err(Error::capture("Invalid capture dimensions"));
    }
    let Some([l, t, r, b]) = crop else {
        return Ok((width, height, pixels.to_vec()));
    };
    if [l, t, r, b]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err(Error::capture("Crop fractions must be within 0..1"));
    }
    let (l, t, r, b) = (
        (l * width as f64) as usize,
        (t * height as f64) as usize,
        (r * width as f64) as usize,
        (b * height as f64) as usize,
    );
    if r <= l || b <= t {
        return Err(Error::capture("Crop produced an empty image"));
    }
    let mut out = Vec::with_capacity((r - l) * (b - t) * 4);
    for y in t..b {
        out.extend_from_slice(&pixels[(y * width + l) * 4..(y * width + r) * 4]);
    }
    Ok((r - l, b - t, out))
}

fn chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) -> Result<()> {
    let len = u32::try_from(data.len()).map_err(|_| Error::capture("PNG chunk too large"))?;
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    bytes.extend_from_slice(&crc.finalize().to_be_bytes());
    Ok(())
}

pub fn encode_png(width: usize, height: usize, bgra: &[u8]) -> Result<Vec<u8>> {
    let expected = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(|| Error::capture("Capture dimensions overflow"))?;
    if width == 0 || height == 0 || bgra.len() != expected {
        return Err(Error::capture("Invalid capture dimensions"));
    }
    let (w, h) = (
        u32::try_from(width).map_err(|_| Error::capture("Image too wide"))?,
        u32::try_from(height).map_err(|_| Error::capture("Image too high"))?,
    );
    let mut rows = Vec::with_capacity(height * (width * 3 + 1));
    for row in bgra.chunks_exact(width * 4) {
        rows.push(0);
        for pixel in row.chunks_exact(4) {
            rows.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
    }
    let mut zip = ZlibEncoder::new(Vec::new(), Compression::new(6));
    zip.write_all(&rows)?;
    let data = zip.finish()?;
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&w.to_be_bytes());
    header.extend_from_slice(&h.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header)?;
    chunk(&mut png, b"IDAT", &data)?;
    chunk(&mut png, b"IEND", &[])?;
    Ok(png)
}

pub fn save_png(path: &Path, width: usize, height: usize, bgra: &[u8]) -> Result<()> {
    let bytes = encode_png(width, height, bgra)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::capture(format!("Screenshot already exists: {}", path.display()))
            } else {
                e.into()
            }
        })?;
    file.write_all(&bytes)?;
    Ok(())
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::click;
    use std::{ffi::c_void, mem::size_of};
    type H = isize;
    #[repr(C)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bits: u16,
        compression: u32,
        image_size: u32,
        xppm: i32,
        yppm: i32,
        colors: u32,
        important: u32,
    }
    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }
    #[link(name = "user32")]
    extern "system" {
        fn IsIconic(hwnd: H) -> i32;
        fn GetWindowRect(hwnd: H, rect: *mut Rect) -> i32;
        fn GetWindowDC(hwnd: H) -> H;
        fn ReleaseDC(hwnd: H, dc: H) -> i32;
        fn PrintWindow(hwnd: H, dc: H, flags: u32) -> i32;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: H) -> H;
        fn CreateCompatibleBitmap(dc: H, w: i32, h: i32) -> H;
        fn SelectObject(dc: H, obj: H) -> H;
        fn BitBlt(
            dest: H,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            src: H,
            sx: i32,
            sy: i32,
            op: u32,
        ) -> i32;
        fn GetDIBits(
            dc: H,
            bmp: H,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(obj: H) -> i32;
        fn DeleteDC(dc: H) -> i32;
    }
    struct CaptureResources {
        hwnd: H,
        source: H,
        memory: H,
        bitmap: H,
        previous: H,
        selected: bool,
    }
    impl Drop for CaptureResources {
        fn drop(&mut self) {
            unsafe {
                if self.selected && self.memory != 0 && self.previous != 0 {
                    SelectObject(self.memory, self.previous);
                }
                if self.bitmap != 0 {
                    DeleteObject(self.bitmap);
                }
                if self.memory != 0 {
                    DeleteDC(self.memory);
                }
                if self.source != 0 {
                    ReleaseDC(self.hwnd, self.source);
                }
            }
        }
    }
    impl CaptureResources {
        fn read(&mut self, w: i32, h: i32) -> Result<Vec<u8>> {
            let len = (w as usize)
                .checked_mul(h as usize)
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(|| Error::capture("Capture dimensions overflow"))?;
            let mut pixels = vec![0u8; len];
            let mut info = BitmapInfo {
                header: BitmapInfoHeader {
                    size: size_of::<BitmapInfoHeader>() as u32,
                    width: w,
                    height: -h,
                    planes: 1,
                    bits: 32,
                    compression: 0,
                    image_size: 0,
                    xppm: 0,
                    yppm: 0,
                    colors: 0,
                    important: 0,
                },
                colors: [0],
            };
            // GetDIBits requires the bitmap to be deselected from its memory DC.
            unsafe {
                SelectObject(self.memory, self.previous);
            }
            self.selected = false;
            let copied = unsafe {
                GetDIBits(
                    self.source,
                    self.bitmap,
                    0,
                    h as u32,
                    pixels.as_mut_ptr().cast(),
                    &mut info,
                    0,
                )
            };
            unsafe {
                SelectObject(self.memory, self.bitmap);
            }
            self.selected = true;
            if copied != h {
                return Err(Error::capture(format!(
                    "GetDIBits returned {copied}/{h} rows"
                )));
            }
            Ok(pixels)
        }
    }
    pub fn capture_civ_window(
        path: &Path,
        crop: Option<[f64; 4]>,
        deadline: Option<f64>,
    ) -> Result<()> {
        check_deadline(deadline)?;
        let hwnd = click::find_civ_window().map_err(|e| Error::capture(e.message))?;
        capture_window(path, crop, deadline, hwnd, None)
    }
    pub fn capture_owned_window(
        path: &Path,
        hwnd: isize,
        pid: u32,
        deadline: Option<f64>,
    ) -> Result<()> {
        capture_window(path, None, deadline, hwnd, Some(pid))
    }
    fn capture_window(
        path: &Path,
        crop: Option<[f64; 4]>,
        deadline: Option<f64>,
        hwnd: H,
        pid: Option<u32>,
    ) -> Result<()> {
        let verify = || -> Result<()> {
            check_deadline(deadline)?;
            if let Some(expected) = pid {
                let mut actual = 0;
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                        hwnd as _,
                        &mut actual,
                    );
                }
                if actual != expected {
                    return Err(Error::capture("pinned startup window changed"));
                }
            }
            Ok(())
        };
        verify()?;
        if unsafe { IsIconic(hwnd) } != 0 {
            return Err(Error::capture(
                "Civ VI window is minimized; restore it before capture",
            ));
        }
        let mut rect = Rect {
            l: 0,
            t: 0,
            r: 0,
            b: 0,
        };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return Err(Error::capture("GetWindowRect failed"));
        }
        let (w, h) = (rect.r - rect.l, rect.b - rect.t);
        if w <= 0 || h <= 0 {
            return Err(Error::capture("Civ VI window has no visible area"));
        }
        let mut resources = CaptureResources {
            hwnd,
            source: 0,
            memory: 0,
            bitmap: 0,
            previous: 0,
            selected: false,
        };
        resources.source = unsafe { GetWindowDC(hwnd) };
        if resources.source == 0 {
            return Err(Error::capture("GetWindowDC failed"));
        }
        resources.memory = unsafe { CreateCompatibleDC(resources.source) };
        if resources.memory == 0 {
            return Err(Error::capture("Could not allocate capture bitmap"));
        }
        resources.bitmap = unsafe { CreateCompatibleBitmap(resources.source, w, h) };
        if resources.bitmap == 0 {
            return Err(Error::capture("Could not allocate capture bitmap"));
        }
        resources.previous = unsafe { SelectObject(resources.memory, resources.bitmap) };
        if resources.previous == 0 || resources.previous == -1 {
            return Err(Error::capture("Could not allocate capture bitmap"));
        }
        resources.selected = true;
        verify()?;
        let printed = unsafe { PrintWindow(hwnd, resources.memory, 2) } != 0;
        check_deadline(deadline)?;
        let mut pixels = if printed {
            resources.read(w, h)?
        } else {
            Vec::new()
        };
        if !printed || pixels.iter().all(|&b| b == 0) {
            verify()?;
            if unsafe {
                BitBlt(
                    resources.memory,
                    0,
                    0,
                    w,
                    h,
                    resources.source,
                    0,
                    0,
                    0x00cc0020,
                )
            } == 0
            {
                return Err(Error::capture("PrintWindow and BitBlt both failed"));
            }
            check_deadline(deadline)?;
            pixels = resources.read(w, h)?;
        }
        if pixels.iter().all(|&b| b == 0) {
            return Err(Error::capture(
                "Capture is blank; make the Civ VI window visible and unobscured",
            ));
        }
        let (cw, ch, cropped) = crop_bgra(w as usize, h as usize, &pixels, crop)?;
        verify()?;
        check_deadline(deadline)?;
        save_png(path, cw, ch, &cropped)?;
        check_deadline(deadline)
    }
}
#[cfg(windows)]
pub use win::capture_civ_window;
#[cfg(windows)]
pub use win::capture_owned_window;
#[cfg(not(windows))]
pub fn capture_civ_window(_: &Path, _: Option<[f64; 4]>, deadline: Option<f64>) -> Result<()> {
    check_deadline(deadline)?;
    Err(Error::capture("Window capture requires Windows"))
}

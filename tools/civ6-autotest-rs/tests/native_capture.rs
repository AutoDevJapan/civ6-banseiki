use civ6_autotest_rs::{capture, ErrorKind};
use flate2::read::ZlibDecoder;
use std::{fs, io::Read};

#[test]
fn crop_uses_window_fractions_and_keeps_bgra_order() {
    let pixels: Vec<_> = (0u8..32).collect(); // four by two pixels
    let (w, h, cropped) = capture::crop_bgra(4, 2, &pixels, Some([0.25, 0.0, 0.75, 1.0])).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(
        cropped,
        [4, 5, 6, 7, 8, 9, 10, 11, 20, 21, 22, 23, 24, 25, 26, 27]
    );
    for crop in [
        [0.5, 0.0, 0.5, 1.0],
        [0.0, 0.0, 1.0, f64::NAN],
        [-0.1, 0.0, 1.0, 1.0],
    ] {
        assert_eq!(
            capture::crop_bgra(4, 2, &pixels, Some(crop))
                .unwrap_err()
                .kind,
            ErrorKind::Capture
        );
    }
}

#[test]
fn png_has_valid_chunks_crc_and_rgb_pixels() {
    let png = capture::encode_png(2, 1, &[3, 2, 1, 255, 6, 5, 4, 0]).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let mut offset = 8;
    let mut kinds = Vec::new();
    while offset < png.len() {
        let len = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data = &png[offset + 8..offset + 8 + len];
        let crc = u32::from_be_bytes(png[offset + 8 + len..offset + 12 + len].try_into().unwrap());
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        assert_eq!(hasher.finalize(), crc);
        kinds.push(kind.to_vec());
        if kind == b"IHDR" {
            assert_eq!(&data[..8], &[0, 0, 0, 2, 0, 0, 0, 1]);
            assert_eq!(&data[8..], [8, 2, 0, 0, 0]);
        }
        if kind == b"IDAT" {
            let mut raw = Vec::new();
            ZlibDecoder::new(data).read_to_end(&mut raw).unwrap();
            assert_eq!(raw, [0, 1, 2, 3, 4, 5, 6]);
        }
        offset += len + 12;
    }
    assert_eq!(
        kinds,
        [b"IHDR".to_vec(), b"IDAT".to_vec(), b"IEND".to_vec()]
    );
}

#[test]
fn screenshot_never_overwrites_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("nested").join("shot.png");
    capture::save_png(&path, 1, 1, &[0, 1, 2, 255]).unwrap();
    let original = fs::read(&path).unwrap();
    assert_eq!(
        capture::save_png(&path, 1, 1, &[9, 9, 9, 255])
            .unwrap_err()
            .kind,
        ErrorKind::Capture
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn invalid_pixel_length_is_rejected() {
    assert_eq!(
        capture::encode_png(2, 2, &[0; 4]).unwrap_err().kind,
        ErrorKind::Capture
    );
}

#[cfg(not(windows))]
#[test]
fn non_windows_capture_fails_explicitly() {
    let e =
        capture::capture_civ_window(std::path::Path::new("unused.png"), None, None).unwrap_err();
    assert!(e.message.contains("Windows"));
}

//! The player's own image as a crosshair. The settings window draws the
//! chosen file (PNG, JPG, WebP, GIF or SVG) on a canvas at the exact size
//! wanted and hands over PNG bytes; here they are checked, kept, and turned
//! into the overlay's premultiplied bitmap. No image library beyond the small
//! `png` decoder: SVG and the other formats are the web view's job.

use crate::core::raster::Bitmap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The largest file the player may choose.
pub const FILE_LIMIT: usize = 5 * 1024 * 1024;
/// The longest side of the image as shown, physical pixels.
pub const SIZE_LIMIT: u32 = 512;
pub const SIZE_MIN: u32 = 4;
/// The kinds of file kept as originals, by extension.
pub const KINDS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "svg"];

/// A decoded image ready to show: straight RGBA, both sides even.
#[derive(Debug, PartialEq)]
pub struct Picture {
    /// The bitmap's size, padded to even sides.
    pub width: u32,
    pub height: u32,
    /// The transparent column on the left and row on top added to make it even.
    pad: (u32, u32),
    rgba: Vec<u8>,
    /// The PNG as stored, for the OBS page.
    pub png: Arc<Vec<u8>>,
}

impl Picture {
    /// The size of the PNG as stored, before padding.
    pub fn stored_size(&self) -> (u32, u32) {
        (self.width - self.pad.0, self.height - self.pad.1)
    }

    /// The overlay bitmap at this opacity (0..255).
    pub fn bitmap(&self, opacity: u8) -> Bitmap {
        let opacity = u32::from(opacity);
        let mut data = Vec::with_capacity(self.rgba.len());
        for pixel in self.rgba.chunks_exact(4) {
            let alpha = (u32::from(pixel[3]) * opacity + 127) / 255;
            let premultiply = |channel: u8| ((u32::from(channel) * alpha + 127) / 255) as u8;
            data.extend_from_slice(&[premultiply(pixel[2]), premultiply(pixel[1]), premultiply(pixel[0]), alpha as u8]);
        }
        Bitmap { width: self.width, height: self.height, data }
    }
}

/// Reads the PNG the settings window made. An odd side gets one transparent
/// column on the left (or row on top), so the bitmap is even like a CS2
/// crosshair's and its middle falls between the screen's two centre pixels;
/// an image's own centre pixel then lands where Scorpio draws a 1 px line.
pub fn decode(png: Vec<u8>) -> Result<Picture, String> {
    if png.len() > FILE_LIMIT {
        return Err("The image is larger than 5 MB.".into());
    }
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(&png[..]), png::Limits { bytes: 4 * 1024 * 1024 });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let unreadable = |error: png::DecodingError| format!("The image could not be read as a PNG ({error}).");
    let header = decoder.read_header_info().map_err(unreadable)?;
    let (width, height) = (header.width, header.height);
    if width == 0 || height == 0 {
        return Err("The image is empty.".into());
    }
    if width > SIZE_LIMIT || height > SIZE_LIMIT {
        return Err(format!("The image is {width} by {height} pixels; the largest is {SIZE_LIMIT} by {SIZE_LIMIT}."));
    }
    let mut reader = decoder.read_info().map_err(unreadable)?;
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("The image is too large.")?];
    let frame = reader.next_frame(&mut buffer).map_err(unreadable)?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("The image's palette could not be read.".into()),
    };
    let (pad_x, pad_y) = (width % 2, height % 2);
    let (out_width, out_height) = (width + pad_x, height + pad_y);
    let mut rgba = vec![0u8; (out_width * out_height * 4) as usize];
    for y in 0..height as usize {
        let row = &buffer[y * frame.line_size..][..width as usize * channels];
        for (x, pixel) in row.chunks_exact(channels).enumerate() {
            let value = match *pixel {
                [g] => [g, g, g, 255],
                [g, a] => [g, g, g, a],
                [r, g, b] => [r, g, b, 255],
                [r, g, b, a] => [r, g, b, a],
                _ => unreachable!("1 to 4 channels"),
            };
            let at = (((y + pad_y as usize) * out_width as usize) + x + pad_x as usize) * 4;
            rgba[at..at + 4].copy_from_slice(&value);
        }
    }
    Ok(Picture { width: out_width, height: out_height, pad: (pad_x, pad_y), rgba, png: Arc::new(png) })
}

/// A picture of one colour, for tests elsewhere.
#[cfg(test)]
pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Picture {
    Picture { width, height, pad: (0, 0), rgba: rgba.repeat((width * height) as usize), png: Arc::new(b"png".to_vec()) }
}

/// Scorpio's own name for a file: 16 hex digits of its content's hash and
/// its kind. The settings file keeps only names of this form, so nothing in
/// it can point outside the images folder.
pub fn file_name(bytes: &[u8], kind: &str) -> String {
    // FNV-1a, 64 bits: a name, not a security measure.
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}.{kind}")
}

pub fn valid_file_name(name: &str) -> bool {
    match name.split_once('.') {
        Some((stem, kind)) => stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) && KINDS.contains(&kind),
        None => false,
    }
}

/// The kind of a file the player chose, from its name; None when Scorpio
/// does not take it.
pub fn kind_of(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    KINDS.iter().find(|kind| **kind == extension).map(|kind| if *kind == "jpeg" { "jpg" } else { kind })
}

/// The folder the images live in.
pub fn folder(config: &Path) -> PathBuf {
    config.join("images")
}

/// Writes a file under its own name and returns the name. The same bytes
/// give the same name, so writing twice is harmless.
pub fn store(folder: &Path, bytes: &[u8], kind: &str) -> std::io::Result<String> {
    let name = file_name(bytes, kind);
    fs::create_dir_all(folder)?;
    let path = folder.join(&name);
    if path.exists() {
        // Fresh again, so a cleanup in the next minute leaves it alone.
        if let Err(error) = fs::File::options().append(true).open(&path).and_then(|file| file.set_modified(std::time::SystemTime::now())) {
            log::warn!("image {name} could not be marked as fresh: {error}");
        }
    } else {
        let temporary = folder.join(format!("{name}.tmp"));
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &path)?;
    }
    Ok(name)
}

pub fn read(folder: &Path, name: &str) -> std::io::Result<Vec<u8>> {
    if !valid_file_name(name) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not an image name of Scorpio's"));
    }
    fs::read(folder.join(name))
}

/// Removes the files no setting uses any more (an image replaced, a size
/// changed) once they are older than `age`, so a file just kept for a
/// setting not yet saved stays. Only names of Scorpio's own form are touched.
pub fn prune(folder: &Path, used: &[String], age: std::time::Duration) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    let now = std::time::SystemTime::now();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let ours = valid_file_name(&name) || name.ends_with(".tmp");
        let modified = entry.metadata().and_then(|meta| meta.modified()).unwrap_or(now);
        let old = now.duration_since(modified).is_ok_and(|elapsed| elapsed >= age);
        if ours && old && !used.contains(&name) {
            if let Err(error) = fs::remove_file(entry.path()) {
                log::warn!("unused image {name} could not be removed: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(data).unwrap();
        out
    }

    #[test]
    fn rgba_is_premultiplied_as_bgra_with_opacity() {
        let picture = decode(encode(2, 2, png::ColorType::Rgba, &[255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255])).unwrap();
        assert_eq!((picture.width, picture.height), (2, 2));
        let full = picture.bitmap(255);
        assert_eq!(&full.data[0..4], &[0, 0, 255, 255], "red as BGRA");
        assert_eq!(&full.data[4..8], &[0, 128, 0, 128], "half green, premultiplied");
        assert_eq!(&full.data[8..12], &[0, 0, 0, 0], "transparent stays black");
        let half = picture.bitmap(128);
        assert_eq!(&half.data[0..4], &[0, 0, 128, 128]);
    }

    #[test]
    fn odd_sides_are_padded_left_and_top_so_the_centre_pixel_stays_put() {
        // 3x1 grey: white in the middle.
        let picture = decode(encode(3, 1, png::ColorType::Grayscale, &[0, 255, 0])).unwrap();
        assert_eq!((picture.width, picture.height), (4, 2));
        assert_eq!(picture.stored_size(), (3, 1));
        let bitmap = picture.bitmap(255);
        // Row 0 is the transparent padding; row 1 is the image, shifted right by one.
        assert!(bitmap.data[..16].iter().all(|&b| b == 0));
        let row: Vec<u8> = bitmap.data[16..].chunks(4).map(|p| p[0]).collect();
        assert_eq!(row, vec![0, 0, 255, 0], "the white centre pixel is at index width / 2");
        assert_eq!(bitmap.data[16 + 3], 0, "the padding is transparent");
    }

    #[test]
    fn bad_input_is_refused_in_plain_words() {
        assert!(decode(b"not a png".to_vec()).unwrap_err().starts_with("The image could not be read"));
        let big = encode(513, 1, png::ColorType::Grayscale, &[0; 513]);
        assert_eq!(decode(big).unwrap_err(), "The image is 513 by 1 pixels; the largest is 512 by 512.");
        assert_eq!(decode(vec![0; FILE_LIMIT + 1]).unwrap_err(), "The image is larger than 5 MB.");
        let mut cut = encode(4, 4, png::ColorType::Rgba, &[7; 64]);
        cut.truncate(cut.len() - 20);
        assert!(decode(cut).is_err());
    }

    #[test]
    fn names_are_scorpios_own_and_storing_prunes_the_rest() {
        let name = file_name(b"abc", "svg");
        assert!(valid_file_name(&name), "{name}");
        for bad in ["../settings.json", "0123456789abcdef.exe", "0123456789ABCDEF.png", "0123456789abcde.png", "0123456789abcdef", ""] {
            assert!(!valid_file_name(bad), "{bad}");
        }
        assert_eq!(kind_of("My Cross.JPEG"), Some("jpg"));
        assert_eq!(kind_of("cross.svg"), Some("svg"));
        assert_eq!(kind_of("cross.bmp"), None);

        let dir = std::env::temp_dir().join(format!("scorpio-picture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let kept = store(&dir, b"one", "png").unwrap();
        let dropped = store(&dir, b"two", "svg").unwrap();
        assert_eq!(store(&dir, b"one", "png").unwrap(), kept);
        fs::write(dir.join("notes.txt"), b"mine").unwrap();
        prune(&dir, &[], std::time::Duration::from_secs(60));
        assert!(read(&dir, &dropped).is_ok(), "a file under a minute old stays, its setting may be on its way");
        prune(&dir, std::slice::from_ref(&kept), std::time::Duration::ZERO);
        assert_eq!(read(&dir, &kept).unwrap(), b"one");
        assert!(read(&dir, &dropped).is_err());
        assert!(dir.join("notes.txt").exists(), "a file Scorpio did not name is left alone");
        assert!(read(&dir, "../notes.txt").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}

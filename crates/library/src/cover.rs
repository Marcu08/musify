use std::fs;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use lofty::file::{TaggedFile, TaggedFileExt};
use lofty::picture::PictureType;

const MAX_COVER_BYTES: u64 = 8 * 1024 * 1024;
const COVER_EDGE: u32 = 512;
const JPEG_QUALITY: u8 = 85;
const FALLBACK_NAMES: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
];

/// Extract an embedded cover (front cover preferred) or fall back to a known
/// `cover.*` / `folder.*` file beside the track. The image is resized and
/// re-encoded to a content-addressed JPEG in the cover cache.
pub(crate) fn extract(
    tagged: Option<&TaggedFile>,
    track_path: &Path,
    cover_dir: &Path,
) -> Option<PathBuf> {
    let tag = tagged.and_then(|file| file.primary_tag().or_else(|| file.first_tag()));
    if let Some(tag) = tag {
        let pictures = tag.pictures();
        let picture = pictures
            .iter()
            .find(|picture| picture.pic_type() == PictureType::CoverFront)
            .or_else(|| pictures.first());
        if let Some(picture) = picture
            && let Some(path) = write_cover(picture.data(), cover_dir)
        {
            return Some(path);
        }
    }

    let parent = track_path.parent()?;
    for name in FALLBACK_NAMES {
        let candidate = parent.join(name);
        let Ok(metadata) = fs::metadata(&candidate) else {
            continue;
        };
        if metadata.len() > MAX_COVER_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&candidate) else {
            continue;
        };
        if let Some(path) = write_cover(&bytes, cover_dir) {
            return Some(path);
        }
    }
    None
}

fn write_cover(bytes: &[u8], cover_dir: &Path) -> Option<PathBuf> {
    if bytes.is_empty() {
        return None;
    }
    let hash = blake3::hash(bytes).to_hex().to_string();
    let destination = cover_dir.join(format!("{hash}.jpg"));
    if destination.exists() {
        return Some(destination);
    }

    let image = image::load_from_memory(bytes).ok()?;
    let thumbnail = image.thumbnail(COVER_EDGE, COVER_EDGE).to_rgb8();
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY);
    encoder.encode_image(&thumbnail).ok()?;

    fs::create_dir_all(cover_dir).ok()?;
    let temporary = cover_dir.join(format!("{hash}.tmp"));
    fs::write(&temporary, encoded).ok()?;
    if fs::rename(&temporary, &destination).is_err() {
        let _ = fs::remove_file(&temporary);
        return None;
    }
    Some(destination)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn writes_a_resized_jpeg_and_reuses_it() {
        let dir = tempfile::tempdir().expect("temp");
        let source = image::RgbImage::from_fn(1024, 768, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 120])
        });
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode png");

        let first = write_cover(&png, dir.path()).expect("first cover");
        let second = write_cover(&png, dir.path()).expect("second cover");
        assert_eq!(first, second);
        let decoded = image::open(&first).expect("open cover");
        assert!(decoded.width() <= COVER_EDGE);
        assert!(decoded.height() <= COVER_EDGE);
    }

    #[test]
    fn invalid_bytes_do_not_create_a_cover() {
        let dir = tempfile::tempdir().expect("temp");
        assert!(write_cover(b"not an image", dir.path()).is_none());
    }
}

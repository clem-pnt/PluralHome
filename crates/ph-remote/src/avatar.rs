//! Photos de profil venues de PluralKit : téléchargées une fois, puis recadrées
//! au carré, réduites et réencodées ici (les métadonnées disparaissent), pour
//! être rangées dans le coffre chiffré comme une photo choisie à la main.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbImage};
use ph_core::members::MAX_AVATAR_BYTES;

/// Même taille que les photos choisies dans l'application.
const SIDE: u32 = 256;
const QUALITY: u8 = 86;
/// Taille maximale acceptée au téléchargement.
pub const MAX_DOWNLOAD: u64 = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 8192;

/// Recadre au centre, réduit à 256 px et réencode (PNG si l'image a de la
/// transparence, JPEG sinon). `None` si l'image est illisible.
#[must_use]
pub fn square(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let side = img.width().min(img.height());
    if side == 0 {
        return None;
    }
    let img = img
        .crop_imm(
            (img.width() - side) / 2,
            (img.height() - side) / 2,
            side,
            side,
        )
        .resize_exact(SIDE, SIDE, FilterType::Lanczos3);

    let rgba = img.to_rgba8();
    if rgba.pixels().any(|p| p.0[3] < 255) {
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(rgba.clone())
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .ok()?;
        if png.len() <= MAX_AVATAR_BYTES {
            return Some(png);
        }
    }
    // Sans transparence (ou PNG trop lourd) : JPEG sur fond blanc.
    let flat = RgbImage::from_fn(SIDE, SIDE, |x, y| {
        let px = rgba.get_pixel(x, y).0;
        let alpha = u16::from(px[3]);
        let blend = |channel: u8| {
            let v = (u16::from(channel) * alpha + 255 * (255 - alpha)) / 255;
            u8::try_from(v).unwrap_or(u8::MAX)
        };
        image::Rgb([blend(px[0]), blend(px[1]), blend(px[2])])
    });
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY)
        .encode_image(&flat)
        .ok()?;
    (jpeg.len() <= MAX_AVATAR_BYTES).then_some(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn encode(img: &DynamicImage, fmt: ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), fmt).unwrap();
        out
    }

    #[test]
    fn opaque_image_becomes_square_jpeg() {
        let img =
            DynamicImage::ImageRgb8(RgbImage::from_pixel(640, 300, image::Rgb([200, 30, 60])));
        let out = square(&encode(&img, ImageFormat::Png)).unwrap();
        assert_eq!(&out[..3], &[0xFF, 0xD8, 0xFF]);
        let back = image::load_from_memory(&out).unwrap();
        assert_eq!((back.width(), back.height()), (SIDE, SIDE));
    }

    #[test]
    fn transparency_is_kept_as_png() {
        let mut img = RgbaImage::from_pixel(100, 120, Rgba([0, 0, 0, 0]));
        img.put_pixel(50, 60, Rgba([10, 200, 10, 255]));
        let out = square(&encode(&DynamicImage::ImageRgba8(img), ImageFormat::Png)).unwrap();
        assert_eq!(&out[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn garbage_is_refused() {
        assert!(square(b"<html>pas une image</html>").is_none());
        assert!(square(&[]).is_none());
    }
}

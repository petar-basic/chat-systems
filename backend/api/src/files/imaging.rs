use std::io::Cursor;

use axum::body::Bytes;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use img_parts::jpeg::{markers, Jpeg};
use img_parts::png::Png;
use img_parts::webp::{WebP, CHUNK_XMP};
use img_parts::ImageEXIF;

pub const MAX_IMAGE_BYTES: usize = 30 * 1024 * 1024;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_SIDE: u32 = 20_000;
const THUMBNAIL_WIDTH: u32 = 720;
const THUMBNAIL_QUALITY: u8 = 82;
const XMP_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PNG_TEXT_CHUNKS: [[u8; 4]; 5] = [*b"eXIf", *b"tEXt", *b"iTXt", *b"zTXt", *b"tIME"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageKind {
    pub fn mime(self) -> &'static str {
        match self {
            ImageKind::Png => "image/png",
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Gif => "image/gif",
            ImageKind::Webp => "image/webp",
        }
    }

    fn format(self) -> ImageFormat {
        match self {
            ImageKind::Png => ImageFormat::Png,
            ImageKind::Jpeg => ImageFormat::Jpeg,
            ImageKind::Gif => ImageFormat::Gif,
            ImageKind::Webp => ImageFormat::WebP,
        }
    }
}

pub fn sniff(head: &[u8]) -> Option<ImageKind> {
    match head {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some(ImageKind::Png),
        [0xFF, 0xD8, 0xFF, ..] => Some(ImageKind::Jpeg),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some(ImageKind::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageKind::Webp),
        _ => None,
    }
}

#[derive(Debug)]
pub struct Thumbnail {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub extension: &'static str,
}

#[derive(Debug)]
pub struct Processed {
    pub original: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub thumbnail: Option<Thumbnail>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImagingError {
    #[error("the image is larger than {MAX_PIXELS} pixels")]
    TooManyPixels,
    #[error("the image could not be decoded: {0}")]
    Decode(#[from] image::ImageError),
    #[error("the image container could not be read: {0}")]
    Container(#[from] img_parts::Error),
}

pub fn process(kind: ImageKind, bytes: Bytes) -> Result<Processed, ImagingError> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes.as_ref()), kind.format());
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (raw_width, raw_height) = decoder.dimensions();
    if u64::from(raw_width) * u64::from(raw_height) > MAX_PIXELS {
        return Err(ImagingError::TooManyPixels);
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut decoded = DynamicImage::from_decoder(decoder)?;
    decoded.apply_orientation(orientation);
    let (width, height) = (decoded.width(), decoded.height());

    let thumbnail = (width > THUMBNAIL_WIDTH)
        .then(|| thumbnail_of(&decoded))
        .transpose()?;
    let original = strip_metadata(kind, bytes, orientation)?;

    Ok(Processed {
        original,
        width,
        height,
        thumbnail,
    })
}

fn thumbnail_of(image: &DynamicImage) -> Result<Thumbnail, ImagingError> {
    let height = (u64::from(image.height()) * u64::from(THUMBNAIL_WIDTH) / u64::from(image.width()))
        .max(1) as u32;
    let small = image.thumbnail_exact(THUMBNAIL_WIDTH, height);
    let mut out = Cursor::new(Vec::new());
    if small.color().has_alpha() {
        small.write_to(&mut out, ImageFormat::Png)?;
        return Ok(Thumbnail {
            bytes: out.into_inner(),
            mime: "image/png",
            extension: "png",
        });
    }
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, THUMBNAIL_QUALITY);
    small.to_rgb8().write_with_encoder(encoder)?;
    Ok(Thumbnail {
        bytes: out.into_inner(),
        mime: "image/jpeg",
        extension: "jpg",
    })
}

fn strip_metadata(
    kind: ImageKind,
    bytes: Bytes,
    orientation: Orientation,
) -> Result<Vec<u8>, ImagingError> {
    let exif = (orientation != Orientation::NoTransforms).then(|| orientation_exif(orientation));
    Ok(match kind {
        ImageKind::Gif => bytes.to_vec(),
        ImageKind::Jpeg => {
            let mut jpeg = Jpeg::from_bytes(bytes)?;
            jpeg.segments_mut().retain(|segment| {
                let xmp =
                    segment.marker() == markers::APP1 && segment.contents().starts_with(XMP_PREFIX);
                !xmp && segment.marker() != markers::COM
            });
            jpeg.set_exif(exif);
            jpeg.encoder().bytes().to_vec()
        }
        ImageKind::Png => {
            let mut png = Png::from_bytes(bytes)?;
            png.chunks_mut()
                .retain(|chunk| !PNG_TEXT_CHUNKS.contains(&chunk.kind()));
            png.set_exif(exif);
            png.encoder().bytes().to_vec()
        }
        ImageKind::Webp => {
            let mut webp = WebP::from_bytes(bytes)?;
            webp.remove_chunks_by_id(CHUNK_XMP);
            webp.set_exif(exif);
            webp.encoder().bytes().to_vec()
        }
    })
}

fn orientation_exif(orientation: Orientation) -> Bytes {
    let mut tiff = Vec::with_capacity(26);
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&u16::from(orientation.to_exif()).to_le_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes());
    Bytes::from(tiff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    const GPS_MARKER: &[u8] = b"GPSLatitude=44.8125N";

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_pixel(width, height, Rgb([200, 30, 30]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Jpeg).expect("encode");
        out.into_inner()
    }

    fn exif_with(orientation: u16) -> Bytes {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&0x010Eu16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&(GPS_MARKER.len() as u32).to_le_bytes());
        tiff.extend_from_slice(&38u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        tiff.extend_from_slice(GPS_MARKER);
        Bytes::from(tiff)
    }

    fn with_exif(bytes: Vec<u8>, exif: Bytes) -> Vec<u8> {
        let mut parsed = Jpeg::from_bytes(Bytes::from(bytes)).expect("jpeg");
        parsed.set_exif(Some(exif));
        parsed.encoder().bytes().to_vec()
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn recognises_images_by_their_bytes_not_their_name() {
        assert_eq!(sniff(&jpeg(2, 2)), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"GIF89a\x01\x00"), Some(ImageKind::Gif));
        assert_eq!(
            sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some(ImageKind::Webp)
        );
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some(ImageKind::Png));
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"%PDF-1.7"), None);
    }

    #[test]
    fn strips_gps_and_keeps_the_orientation() {
        let photo = with_exif(jpeg(40, 20), exif_with(6));
        assert!(
            contains(&photo, GPS_MARKER),
            "the fixture carries a location"
        );

        let processed = process(ImageKind::Jpeg, Bytes::from(photo)).expect("process");

        assert!(
            !contains(&processed.original, GPS_MARKER),
            "the location is gone"
        );
        let kept = Jpeg::from_bytes(Bytes::from(processed.original.clone()))
            .expect("still a jpeg")
            .exif()
            .expect("an orientation-only block");
        assert_eq!(
            Orientation::from_exif_chunk(&kept),
            Some(Orientation::Rotate90)
        );
        assert_eq!(
            (processed.width, processed.height),
            (20, 40),
            "sizes are as displayed"
        );
        image::load_from_memory(&processed.original).expect("the original still decodes");
    }

    #[test]
    fn an_upright_photo_keeps_no_exif_at_all() {
        let processed = process(
            ImageKind::Jpeg,
            Bytes::from(with_exif(jpeg(10, 10), exif_with(1))),
        )
        .expect("process");
        let parsed = Jpeg::from_bytes(Bytes::from(processed.original)).expect("jpeg");
        assert!(parsed.exif().is_none());
        assert!(
            processed.thumbnail.is_none(),
            "a small image is its own thumbnail"
        );
    }

    #[test]
    fn a_wide_image_gets_a_thumbnail_at_the_card_width() {
        let processed = process(ImageKind::Jpeg, Bytes::from(jpeg(1600, 800))).expect("process");
        let thumbnail = processed.thumbnail.expect("thumbnail");
        assert_eq!(thumbnail.mime, "image/jpeg");
        let decoded = image::load_from_memory(&thumbnail.bytes).expect("decode");
        assert_eq!((decoded.width(), decoded.height()), (720, 360));
    }

    #[test]
    fn a_transparent_image_keeps_its_transparency_in_the_thumbnail() {
        let image = RgbaImage::from_pixel(1000, 500, Rgba([0, 0, 0, 0]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).expect("encode");
        let processed = process(ImageKind::Png, Bytes::from(out.into_inner())).expect("process");
        assert_eq!(processed.thumbnail.expect("thumbnail").mime, "image/png");
    }

    #[test]
    fn refuses_to_decode_an_absurd_canvas() {
        let mut header = Vec::new();
        header.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        header.extend_from_slice(&13u32.to_be_bytes());
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&10_000u32.to_be_bytes());
        header.extend_from_slice(&10_000u32.to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        header.extend_from_slice(&[0, 0, 0, 0]);
        assert!(process(ImageKind::Png, Bytes::from(header)).is_err());
    }
}

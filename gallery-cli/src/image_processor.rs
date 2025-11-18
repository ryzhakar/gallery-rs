use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use std::io::Cursor;
use std::path::Path;

pub struct ProcessedImage {
    pub original: Vec<u8>,
    pub preview: Vec<u8>,
    pub thumbnail: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

const THUMBNAIL_SIZE: u32 = 400;
const PREVIEW_SIZE: u32 = 2048;
const JPEG_QUALITY: u8 = 92; // High quality for film grain preservation

pub fn process_image(path: &Path) -> Result<ProcessedImage> {
    tracing::info!("Processing image: {}", path.display());

    // Load the original image
    let img = image::open(path)
        .context(format!("Failed to open image: {}", path.display()))?;

    let (width, height) = img.dimensions();

    // Keep original as high-quality JPEG
    let original = encode_jpeg(&img, JPEG_QUALITY)?;

    // Create preview (2048px max dimension) - for lightbox initial load
    let preview = create_resized_jpeg(&img, PREVIEW_SIZE, 90)?;

    // Create thumbnail (400px max dimension) - for grid
    let thumbnail = create_resized_jpeg(&img, THUMBNAIL_SIZE, 85)?;

    Ok(ProcessedImage {
        original,
        preview,
        thumbnail,
        width,
        height,
    })
}

fn create_resized_jpeg(img: &DynamicImage, max_size: u32, quality: u8) -> Result<Vec<u8>> {
    let (width, height) = img.dimensions();

    // Only resize if larger than target
    let resized = if width > max_size || height > max_size {
        img.resize(max_size, max_size, FilterType::Lanczos3)
    } else {
        img.clone()
    };

    encode_jpeg(&resized, quality)
}

fn encode_jpeg(img: &DynamicImage, _quality: u8) -> Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());

    img.write_to(&mut buffer, ImageFormat::Jpeg)
        .context("Failed to encode JPEG")?;

    // TODO: Use quality parameter with a JPEG encoder that supports it
    // For now, using standard image crate encoding with default quality
    Ok(buffer.into_inner())
}

pub fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or("").to_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "tiff" | "tif" | "bmp" | "webp"
        )
    } else {
        false
    }
}

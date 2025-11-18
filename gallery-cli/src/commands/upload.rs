use anyhow::Result;
use gallery_core::{AlbumManifest, ImageInfo, S3Client};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::image_processor::{is_image_file, process_image};

pub async fn execute(paths: Vec<String>, name: String, bucket: String) -> Result<()> {
    tracing::info!("Creating album: {}", name);

    // Initialize S3 client
    let s3 = S3Client::new(bucket).await?;

    // Create album manifest
    let mut manifest = AlbumManifest::new(name.clone());
    let album_id = manifest.id.clone();

    tracing::info!("Album ID: {}", album_id);

    // Collect all image paths
    let image_paths = collect_image_paths(paths)?;

    if image_paths.is_empty() {
        anyhow::bail!("No images found in the provided paths");
    }

    tracing::info!("Found {} images to process", image_paths.len());

    // Process and upload each image
    for (index, path) in image_paths.iter().enumerate() {
        let image_id = Uuid::new_v4().to_string();

        tracing::info!(
            "[{}/{}] Processing: {}",
            index + 1,
            image_paths.len(),
            path.display()
        );

        // Process image (resize, thumbnails, etc.)
        let processed = process_image(path)?;

        // Upload original
        let original_key = format!("{}/originals/{}.jpg", album_id, image_id);
        tracing::info!("  Uploading original...");
        s3.upload_bytes(processed.original, &original_key).await?;

        // Upload preview
        let preview_key = format!("{}/previews/{}.jpg", album_id, image_id);
        tracing::info!("  Uploading preview...");
        s3.upload_bytes(processed.preview, &preview_key).await?;

        // Upload thumbnail
        let thumbnail_key = format!("{}/thumbnails/{}.jpg", album_id, image_id);
        tracing::info!("  Uploading thumbnail...");
        s3.upload_bytes(processed.thumbnail, &thumbnail_key).await?;

        // Add to manifest
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        manifest.add_image(ImageInfo::new(
            filename,
            processed.width,
            processed.height,
            &album_id,
            &image_id,
        ));
    }

    // Upload manifest
    tracing::info!("Uploading manifest...");
    let manifest_json = manifest.to_json()?;
    let manifest_key = format!("{}/manifest.json", album_id);
    s3.upload_bytes(manifest_json.into_bytes(), &manifest_key)
        .await?;

    println!("\n✓ Album created successfully!");
    println!("Album ID: {}", album_id);
    println!("Total images: {}", manifest.images.len());
    println!("\nAccess your gallery at: https://your-domain.com/gallery/{}", album_id);

    Ok(())
}

fn collect_image_paths(paths: Vec<String>) -> Result<Vec<PathBuf>> {
    let mut image_paths = Vec::new();

    for path_str in paths {
        let path = Path::new(&path_str);

        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        if path.is_file() {
            if is_image_file(path) {
                image_paths.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            // Walk directory and collect all images
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() && is_image_file(entry_path) {
                    image_paths.push(entry_path.to_path_buf());
                }
            }
        }
    }

    // Sort for consistent ordering
    image_paths.sort();

    Ok(image_paths)
}

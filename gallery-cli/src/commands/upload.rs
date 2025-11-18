use anyhow::Result;
use gallery_core::{AlbumManifest, ImageInfo, S3Client};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::image_processor::{is_image_file, process_image, ProcessedImage};

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

    // Process images in parallel using rayon (CPU-bound work)
    tracing::info!("Processing images in parallel...");
    let processed_results: Vec<_> = image_paths
        .par_iter()
        .enumerate()
        .map(|(index, path)| {
            let image_id = Uuid::new_v4().to_string();
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            tracing::info!(
                "[{}/{}] Processing: {}",
                index + 1,
                image_paths.len(),
                filename
            );

            let processed = process_image(path)?;

            Ok::<_, anyhow::Error>((image_id, filename, processed))
        })
        .collect::<Result<Vec<_>>>()?;

    tracing::info!("All images processed. Uploading to S3 concurrently...");

    // Upload all images concurrently using tokio (I/O-bound work)
    let mut upload_tasks = Vec::new();

    for (image_id, filename, processed) in processed_results {
        let s3_clone = s3.clone();
        let album_id_clone = album_id.clone();

        // Spawn concurrent upload task
        let task = tokio::spawn(async move {
            upload_image_to_s3(s3_clone, album_id_clone, image_id, filename, processed).await
        });

        upload_tasks.push(task);
    }

    // Wait for all uploads to complete and collect results
    for task in upload_tasks {
        let image_info = task.await??;
        manifest.add_image(image_info);
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

async fn upload_image_to_s3(
    s3: S3Client,
    album_id: String,
    image_id: String,
    filename: String,
    processed: ProcessedImage,
) -> Result<ImageInfo> {
    // Upload original
    let original_key = format!("{}/originals/{}.jpg", album_id, image_id);
    s3.upload_bytes(processed.original, &original_key).await?;

    // Upload preview
    let preview_key = format!("{}/previews/{}.jpg", album_id, image_id);
    s3.upload_bytes(processed.preview, &preview_key).await?;

    // Upload thumbnail
    let thumbnail_key = format!("{}/thumbnails/{}.jpg", album_id, image_id);
    s3.upload_bytes(processed.thumbnail, &thumbnail_key).await?;

    tracing::info!("✓ Uploaded: {}", filename);

    Ok(ImageInfo::new(
        filename,
        processed.width,
        processed.height,
        &album_id,
        &image_id,
    ))
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

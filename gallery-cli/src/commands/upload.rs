use anyhow::Result;
use gallery_core::{AlbumManifest, ImageInfo, S3Client};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::image_processor::{is_image_file, process_image, ProcessedImage};

pub async fn execute(paths: Vec<String>, name: String, bucket: String) -> Result<()> {
    // Initialize S3 client
    let s3 = S3Client::new(bucket).await?;

    // Collect all image paths
    let image_paths = collect_image_paths(paths)?;

    if image_paths.is_empty() {
        anyhow::bail!("No images found in the provided paths");
    }

    // Create deterministic album ID from the sorted list of image paths
    // This ensures the same set of images always produces the same album ID
    let album_id = compute_album_id(&image_paths);

    println!("Album: {}", name);
    println!("Album ID: {}", album_id);
    println!("Image set size: {}\n", image_paths.len());

    // Check if this album already exists
    let manifest_key = format!("{}/manifest.json", album_id);
    let existing_manifest = if s3.object_exists(&manifest_key).await? {
        println!("✓ Found existing album with this image set");
        println!("  Checking which images need to be uploaded...\n");

        let manifest_data = s3.download_file(&manifest_key).await?;
        let manifest_json = String::from_utf8(manifest_data)?;
        Some(AlbumManifest::from_json(&manifest_json)?)
    } else {
        println!("✓ New album - will upload all images\n");
        None
    };

    // Build hash map of existing images if resume mode
    let existing_images: HashMap<String, ImageInfo> = existing_manifest
        .as_ref()
        .map(|m| {
            m.images
                .iter()
                .map(|img| (img.file_hash.clone(), img.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Process images in parallel using rayon (CPU-bound work)
    // For each image: hash file, check if exists, process if needed
    let process_pb = ProgressBar::new(image_paths.len() as u64);
    process_pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .expect("Invalid progress bar template")
            .progress_chars("█▓▒░ "),
    );
    process_pb.set_message("Processing images...");

    let pb = Arc::new(process_pb);
    let existing_images = Arc::new(existing_images);

    enum ProcessResult {
        Existing(ImageInfo),
        New(String, String, String, ProcessedImage), // image_id, filename, hash, processed
    }

    let process_results: Vec<_> = image_paths
        .par_iter()
        .map(|path| {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Hash the file content
            let file_hash = hash_file(path)?;

            // Check if this image already exists in the album
            if let Some(existing_info) = existing_images.get(&file_hash) {
                pb.inc(1);
                pb.set_message(format!("Skipped (exists): {}", filename));
                return Ok::<_, anyhow::Error>(ProcessResult::Existing(existing_info.clone()));
            }

            // Process the image (new or changed)
            let image_id = Uuid::new_v4().to_string();
            let processed = process_image(path)?;

            pb.inc(1);
            pb.set_message(format!("Processed: {}", filename));

            Ok(ProcessResult::New(image_id, filename, file_hash, processed))
        })
        .collect::<Result<Vec<_>>>()?;

    pb.finish_with_message("Processing complete");
    println!();

    // Separate existing images from new ones
    let mut reused_images = Vec::new();
    let mut new_images = Vec::new();

    for result in process_results {
        match result {
            ProcessResult::Existing(image_info) => reused_images.push(image_info),
            ProcessResult::New(image_id, filename, file_hash, processed) => {
                new_images.push((image_id, filename, file_hash, processed));
            }
        }
    }

    println!(
        "Images: {} total ({} already uploaded, {} to upload)\n",
        image_paths.len(),
        reused_images.len(),
        new_images.len()
    );

    // Upload new images concurrently using tokio (I/O-bound work)
    let mut uploaded_images = Vec::new();

    if !new_images.is_empty() {
        let upload_pb = ProgressBar::new(new_images.len() as u64);
        upload_pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}")
                .expect("Invalid progress bar template")
                .progress_chars("█▓▒░ "),
        );
        upload_pb.set_message("Uploading to S3...");

        let mut upload_tasks = Vec::new();

        for (image_id, filename, file_hash, processed) in new_images {
            let s3_clone = s3.clone();
            let album_id_clone = album_id.clone();
            let pb_clone = upload_pb.clone();

            // Spawn concurrent upload task
            let task = tokio::spawn(async move {
                let result =
                    upload_image_to_s3(s3_clone, album_id_clone, image_id, filename.clone(), file_hash, processed)
                        .await;
                pb_clone.inc(1);
                pb_clone.set_message(format!("Uploaded: {}", filename));
                result
            });

            upload_tasks.push(task);
        }

        // Wait for all uploads to complete and collect results
        for task in upload_tasks {
            let image_info = task.await??;
            uploaded_images.push(image_info);
        }

        upload_pb.finish_with_message("All new images uploaded");
        println!();
    }

    // Create new manifest with all images (reused + newly uploaded)
    let mut manifest = AlbumManifest::with_id(name, album_id.clone());

    // Add all images to manifest
    for image in reused_images.into_iter().chain(uploaded_images.into_iter()) {
        manifest.add_image(image);
    }

    // Upload manifest
    let manifest_json = manifest.to_json()?;
    let manifest_key = format!("{}/manifest.json", album_id);
    s3.upload_bytes(manifest_json.into_bytes(), &manifest_key)
        .await?;

    println!("✓ Album complete!");
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
    file_hash: String,
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

    Ok(ImageInfo::new(
        filename,
        processed.width,
        processed.height,
        file_hash,
        &album_id,
        &image_id,
    ))
}

/// Compute a deterministic album ID from the set of image paths
fn compute_album_id(image_paths: &[PathBuf]) -> String {
    let mut hasher = Sha256::new();

    // Hash the sorted list of image paths (canonicalized representations)
    // This ensures the same set of images always produces the same ID
    for path in image_paths {
        // Use the path as a string for hashing
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"\n"); // Separator
    }

    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string() // Use first 16 chars
}

/// Hash a file's content
fn hash_file(path: &Path) -> Result<String> {
    let file_content = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&file_content);
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
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

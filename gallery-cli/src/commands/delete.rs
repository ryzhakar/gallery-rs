use anyhow::Result;
use gallery_core::S3Client;

pub async fn execute(album_id: String, bucket: String) -> Result<()> {
    tracing::info!("Deleting album: {}", album_id);

    // Initialize S3 client
    let s3 = S3Client::new(bucket).await?;

    // Check if manifest exists
    let manifest_key = format!("{album_id}/manifest.json");
    if !s3.object_exists(&manifest_key).await? {
        anyhow::bail!("Album not found: {album_id}");
    }

    // Delete all objects with the album prefix
    tracing::info!("Deleting all album files...");
    s3.delete_prefix(&format!("{album_id}/")).await?;

    println!("✓ Album deleted successfully: {album_id}");

    Ok(())
}

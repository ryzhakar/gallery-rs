use anyhow::Result;
use gallery_core::S3Client;

#[derive(Clone)]
pub struct AppState {
    pub s3: S3Client,
}

impl AppState {
    pub async fn new(bucket: String) -> Result<Self> {
        let s3 = S3Client::new(bucket).await?;

        Ok(Self { s3 })
    }
}

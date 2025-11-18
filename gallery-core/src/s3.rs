use anyhow::{Context, Result};
use aws_sdk_s3::{
    primitives::{ByteStream, DateTime},
    Client,
};
use std::path::Path;

#[derive(Clone)]
pub struct S3Client {
    client: Client,
    bucket: String,
}

impl S3Client {
    pub async fn new(bucket: String) -> Result<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

        // If AWS_ENDPOINT_URL is set, use it (for MinIO/LocalStack/etc)
        if let Ok(endpoint_url) = std::env::var("AWS_ENDPOINT_URL") {
            config_loader = config_loader.endpoint_url(&endpoint_url);
        }

        let config = config_loader.load().await;
        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&config);

        // For S3-compatible services, force path-style addressing
        if std::env::var("AWS_ENDPOINT_URL").is_ok() {
            s3_config_builder = s3_config_builder.force_path_style(true);
        }

        let s3_config = s3_config_builder.build();
        let client = Client::from_conf(s3_config);

        Ok(Self { client, bucket })
    }

    /// Upload a file to S3
    pub async fn upload_file(&self, local_path: &Path, s3_key: &str) -> Result<()> {
        tracing::debug!("S3 PUT: bucket={}, key={}, local_path={:?}", self.bucket, s3_key, local_path);

        let body = ByteStream::from_path(local_path)
            .await
            .context("Failed to read file")?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .body(body)
            .content_type(Self::guess_content_type(s3_key))
            .send()
            .await
            .context("Failed to upload to S3")?;

        tracing::debug!("S3 PUT success: key={}", s3_key);
        Ok(())
    }

    /// Upload bytes to S3
    pub async fn upload_bytes(&self, data: Vec<u8>, s3_key: &str, expires: Option<DateTime>) -> Result<()> {
        tracing::debug!("S3 PUT (bytes): bucket={}, key={}, size={} bytes", self.bucket, s3_key, data.len());

        let body = ByteStream::from(data);

        let mut request = self.client
            .put_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .body(body)
            .content_type(Self::guess_content_type(s3_key));

        if let Some(expires_at) = expires {
            request = request.expires(expires_at);
        }

        request
            .send()
            .await
            .context("Failed to upload to S3")?;

        tracing::debug!("S3 PUT (bytes) success: key={}", s3_key);
        Ok(())
    }

    /// Download a file from S3
    pub async fn download_file(&self, s3_key: &str) -> Result<Vec<u8>> {
        tracing::debug!("S3 GET: bucket={}, key={}", self.bucket, s3_key);

        let response = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
            .context("Failed to download from S3")?;

        let data = response
            .body
            .collect()
            .await
            .context("Failed to read S3 object body")?;

        let bytes = data.to_vec();
        tracing::debug!("S3 GET success: key={}, size={} bytes", s3_key, bytes.len());
        Ok(bytes)
    }

    /// Delete all objects with a prefix (album deletion)
    pub async fn delete_prefix(&self, prefix: &str) -> Result<()> {
        // List all objects with the prefix
        let objects = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .send()
            .await
            .context("Failed to list objects")?;

        // Delete each object
        if let Some(contents) = objects.contents {
            for object in contents {
                if let Some(key) = object.key {
                    self.client
                        .delete_object()
                        .bucket(&self.bucket)
                        .key(&key)
                        .send()
                        .await
                        .context(format!("Failed to delete {key}"))?;
                }
            }
        }

        Ok(())
    }

    /// Get public URL for an object (if bucket is public)
    pub fn get_public_url(&self, s3_key: &str) -> String {
        format!(
            "https://{}.s3.amazonaws.com/{}",
            self.bucket, s3_key
        )
    }

    /// Check if object exists
    pub async fn object_exists(&self, s3_key: &str) -> Result<bool> {
        match self.client
            .head_object()
            .bucket(&self.bucket)
            .key(s3_key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn guess_content_type(key: &str) -> &'static str {
        if key.ends_with(".jpg") || key.ends_with(".jpeg") {
            "image/jpeg"
        } else if key.ends_with(".png") {
            "image/png"
        } else if key.ends_with(".json") {
            "application/json"
        } else {
            "application/octet-stream"
        }
    }
}

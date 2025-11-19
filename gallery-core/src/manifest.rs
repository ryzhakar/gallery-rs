use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumManifest {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub images: Vec<ImageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub original_filename: String,
    pub width: u32,
    pub height: u32,
    pub file_hash: String,
    pub thumbnail_path: String,
    pub preview_path: String,
    pub original_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_url: Option<String>,
}

impl AlbumManifest {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: chrono::Utc::now().to_rfc3339(),
            images: Vec::new(),
        }
    }

    pub fn with_id(name: String, id: String) -> Self {
        Self {
            id,
            name,
            created_at: chrono::Utc::now().to_rfc3339(),
            images: Vec::new(),
        }
    }

    pub fn add_image(&mut self, info: ImageInfo) {
        self.images.push(info);
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

impl ImageInfo {
    pub fn new(
        original_filename: String,
        width: u32,
        height: u32,
        file_hash: String,
        _album_id: &str,
        image_id: &str,
    ) -> Self {
        Self {
            id: image_id.to_string(),
            original_filename,
            width,
            height,
            file_hash,
            thumbnail_path: format!("thumbnails/{image_id}.jpg"),
            preview_path: format!("previews/{image_id}.jpg"),
            original_path: format!("originals/{image_id}.jpg"),
            thumbnail_url: None,
            preview_url: None,
            original_url: None,
        }
    }
}

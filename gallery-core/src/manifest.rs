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
    pub thumbnail_path: String,
    pub preview_path: String,
    pub original_path: String,
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
        album_id: &str,
        image_id: &str,
    ) -> Self {
        Self {
            id: image_id.to_string(),
            original_filename,
            width,
            height,
            thumbnail_path: format!("{}/thumbnails/{}.jpg", album_id, image_id),
            preview_path: format!("{}/previews/{}.jpg", album_id, image_id),
            original_path: format!("{}/originals/{}.jpg", album_id, image_id),
        }
    }
}

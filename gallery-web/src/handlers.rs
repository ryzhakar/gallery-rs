use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use gallery_core::AlbumManifest;

use crate::state::AppState;

/// Index page
pub async fn index() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Film Gallery</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            max-width: 800px;
            margin: 100px auto;
            padding: 20px;
            text-align: center;
        }
        h1 {
            font-size: 3rem;
            font-weight: 300;
            margin-bottom: 1rem;
        }
        p {
            font-size: 1.2rem;
            color: #666;
        }
    </style>
</head>
<body>
    <h1>Film Gallery</h1>
    <p>Access your private gallery using the link provided.</p>
</body>
</html>
        "#,
    )
}

/// Gallery page
pub async fn gallery(
    State(state): State<AppState>,
    Path(album_id): Path<String>,
) -> Html<String> {
    tracing::info!("Gallery page request: album_id={}", album_id);

    // Verify album exists by checking manifest
    let manifest_key = format!("{album_id}/manifest.json");
    let manifest_data = match state.s3.download_file(&manifest_key).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to fetch manifest for album {}: {:?}", album_id, e);
            return Html(generate_404_html());
        }
    };

    let manifest_json = match String::from_utf8(manifest_data) {
        Ok(json) => json,
        Err(_) => return Html(generate_404_html()),
    };

    let manifest: AlbumManifest = match serde_json::from_str(&manifest_json) {
        Ok(m) => m,
        Err(_) => return Html(generate_404_html()),
    };

    // Generate HTML
    let html = generate_gallery_html(&album_id, &manifest);

    Html(html)
}

/// Get album manifest JSON
pub async fn get_manifest(
    State(state): State<AppState>,
    Path(album_id): Path<String>,
) -> Result<Json<AlbumManifest>, StatusCode> {
    tracing::info!("Manifest API request: album_id={}", album_id);

    let manifest_key = format!("{album_id}/manifest.json");
    let manifest_data = state
        .s3
        .download_file(&manifest_key)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch manifest for album {}: {:?}", album_id, e);
            StatusCode::NOT_FOUND
        })?;

    let manifest_json = String::from_utf8(manifest_data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let manifest: AlbumManifest =
        serde_json::from_str(&manifest_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(manifest))
}

/// Get image from S3
pub async fn get_image(
    State(state): State<AppState>,
    Path((album_id, path)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    tracing::info!("Image request: album_id={}, path={}", album_id, path);

    let s3_key = format!("{album_id}/{path}");
    tracing::debug!("Computed S3 key: {}", s3_key);

    let image_data = state
        .s3
        .download_file(&s3_key)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch image {}: {:?}", s3_key, e);
            StatusCode::NOT_FOUND
        })?;

    // Determine content type
    let content_type = if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    };

    tracing::debug!("Serving image: s3_key={}, content_type={}, size={} bytes", s3_key, content_type, image_data.len());
    Ok(([(header::CONTENT_TYPE, content_type)], image_data).into_response())
}

fn generate_gallery_html(album_id: &str, manifest: &AlbumManifest) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{album_name} - Film Gallery</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #ffffff;
            color: #333;
            line-height: 1.6;
        }}

        .header {{
            padding: 40px 20px;
            text-align: center;
            border-bottom: 1px solid #eee;
        }}

        .header h1 {{
            font-size: 2.5rem;
            font-weight: 300;
            margin-bottom: 10px;
        }}

        .header p {{
            color: #666;
            font-size: 0.9rem;
        }}

        .gallery-container {{
            max-width: 1400px;
            margin: 0 auto;
            padding: 40px 20px;
        }}

        /* Centered justified gallery layout */
        .bento-grid {{
            display: flex;
            flex-wrap: wrap;
            justify-content: center;
            gap: 15px;
            align-items: center;
        }}

        .bento-item {{
            position: relative;
            cursor: pointer;
            background: #f5f5f5;
            border-radius: 4px;
            transition: transform 0.2s ease;
            flex: 0 0 auto;
            max-height: 300px;
        }}

        .bento-item:hover {{
            transform: translateY(-4px);
            box-shadow: 0 8px 20px rgba(0,0,0,0.1);
        }}

        .bento-item img {{
            display: block;
            height: 300px;
            width: auto;
            object-fit: contain;
            border-radius: 4px;
        }}

        /* Lightbox */
        .lightbox {{
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.95);
            z-index: 1000;
            align-items: center;
            justify-content: center;
        }}

        .lightbox.active {{
            display: flex;
        }}

        .lightbox-content {{
            position: relative;
            max-width: 95%;
            max-height: 95%;
            display: flex;
            align-items: center;
            justify-content: center;
        }}

        .lightbox-image {{
            max-width: 100%;
            max-height: 95vh;
            object-fit: contain;
        }}

        .lightbox-controls {{
            position: fixed;
            top: 20px;
            right: 20px;
            display: flex;
            gap: 10px;
            z-index: 1001;
        }}

        .lightbox-btn {{
            background: rgba(255, 255, 255, 0.9);
            border: none;
            padding: 12px 20px;
            cursor: pointer;
            font-size: 1rem;
            border-radius: 4px;
            transition: background 0.2s;
        }}

        .lightbox-btn:hover {{
            background: #fff;
        }}

        .close-btn {{
            position: fixed;
            top: 20px;
            left: 20px;
            background: rgba(255, 255, 255, 0.9);
            border: none;
            width: 40px;
            height: 40px;
            cursor: pointer;
            font-size: 1.5rem;
            border-radius: 4px;
            z-index: 1001;
        }}

        .close-btn:hover {{
            background: #fff;
        }}

        .loading {{
            color: #fff;
            font-size: 1.2rem;
        }}

        @media (max-width: 768px) {{
            .bento-grid {{
                flex-direction: column;
                align-items: stretch;
            }}

            .bento-item {{
                max-height: none;
                width: 100%;
            }}

            .bento-item img {{
                width: 100%;
                height: auto;
            }}
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>{album_name}</h1>
        <p>{image_count} photographs</p>
    </div>

    <div class="gallery-container">
        <div class="bento-grid" id="gallery">
            {thumbnails}
        </div>
    </div>

    <div class="lightbox" id="lightbox">
        <button class="close-btn" onclick="closeLightbox()">&times;</button>
        <div class="lightbox-controls">
            <button class="lightbox-btn" onclick="downloadImage()">Download</button>
        </div>
        <div class="lightbox-content">
            <img class="lightbox-image" id="lightbox-img" src="" alt="">
        </div>
    </div>

    <script>
        const albumId = '{album_id}';
        const images = {images_json};
        let currentImageIndex = 0;

        function openLightbox(index) {{
            currentImageIndex = index;
            const image = images[index];
            const lightbox = document.getElementById('lightbox');
            const lightboxImg = document.getElementById('lightbox-img');

            // Show loading
            lightboxImg.src = `/api/album/${{albumId}}/image/${{image.preview_path}}`;
            lightbox.classList.add('active');

            // Preload full resolution
            const fullImg = new Image();
            fullImg.onload = () => {{
                lightboxImg.src = fullImg.src;
            }};
            fullImg.src = `/api/album/${{albumId}}/image/${{image.original_path}}`;
        }}

        function closeLightbox() {{
            document.getElementById('lightbox').classList.remove('active');
        }}

        function downloadImage() {{
            const image = images[currentImageIndex];
            const link = document.createElement('a');
            link.href = `/api/album/${{albumId}}/image/${{image.original_path}}`;
            link.download = image.original_filename;
            document.body.appendChild(link);
            link.click();
            document.body.removeChild(link);
        }}

        // Close on escape key
        document.addEventListener('keydown', (e) => {{
            if (e.key === 'Escape') closeLightbox();
        }});

        // Close on background click
        document.getElementById('lightbox').addEventListener('click', (e) => {{
            if (e.target.id === 'lightbox') closeLightbox();
        }});
    </script>
</body>
</html>"#,
        album_name = html_escape(&manifest.name),
        album_id = album_id,
        image_count = manifest.images.len(),
        thumbnails = generate_thumbnails_html(album_id, manifest),
        images_json = serde_json::to_string(&manifest.images).unwrap_or_else(|_| "[]".to_string()),
    )
}

fn generate_thumbnails_html(album_id: &str, manifest: &AlbumManifest) -> String {
    manifest
        .images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            format!(
                r#"<div class="bento-item" onclick="openLightbox({index})">
                <img src="/api/album/{album_id}/image/{thumbnail_path}" alt="{filename}" loading="lazy">
            </div>"#,
                index = index,
                album_id = album_id,
                thumbnail_path = image.thumbnail_path,
                filename = html_escape(&image.original_filename),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn generate_404_html() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Gallery Not Found</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: #ffffff;
            color: #333;
        }
        .container {
            text-align: center;
            padding: 40px 20px;
            max-width: 500px;
        }
        h1 {
            font-size: 6rem;
            font-weight: 300;
            margin: 0;
            color: #999;
        }
        p {
            font-size: 1.2rem;
            margin: 20px 0;
            color: #666;
        }
        a {
            color: #333;
            text-decoration: none;
            border-bottom: 1px solid #333;
        }
        a:hover {
            border-bottom: 2px solid #333;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>404</h1>
        <p>This gallery doesn't exist or has expired.</p>
        <p><a href="/">Return home</a></p>
    </div>
</body>
</html>"#.to_string()
}

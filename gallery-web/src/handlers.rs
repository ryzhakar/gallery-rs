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
    let mut manifest: AlbumManifest =
        serde_json::from_str(&manifest_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate presigned URLs for all images (valid for 1 hour)
    let expires_in = std::time::Duration::from_secs(3600);
    for image in &mut manifest.images {
        let thumbnail_key = format!("{album_id}/{}", image.thumbnail_path);
        let preview_key = format!("{album_id}/{}", image.preview_path);
        let original_key = format!("{album_id}/{}", image.original_path);

        image.thumbnail_url = state.s3.generate_presigned_url(&thumbnail_key, expires_in).await.ok();
        image.preview_url = state.s3.generate_presigned_url(&preview_key, expires_in).await.ok();
        image.original_url = state.s3.generate_presigned_url(&original_key, expires_in).await.ok();
    }

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
            background: rgba(0, 0, 0, 0.97);
            z-index: 1000;
            align-items: center;
            justify-content: center;
            opacity: 0;
            transition: opacity 0.3s ease;
        }}

        .lightbox.active {{
            display: flex;
            opacity: 1;
        }}

        .lightbox-content {{
            position: relative;
            max-width: 90%;
            max-height: 90%;
            display: flex;
            align-items: center;
            justify-content: center;
        }}

        .lightbox-image {{
            max-width: 100%;
            max-height: 90vh;
            object-fit: contain;
            user-select: none;
        }}

        /* Navigation arrows */
        .nav-btn {{
            position: fixed;
            top: 50%;
            transform: translateY(-50%);
            background: rgba(255, 255, 255, 0.1);
            border: none;
            width: 60px;
            height: 60px;
            cursor: pointer;
            font-size: 2rem;
            color: white;
            border-radius: 50%;
            z-index: 1001;
            transition: all 0.2s ease;
            backdrop-filter: blur(10px);
            display: flex;
            align-items: center;
            justify-content: center;
        }}

        .nav-btn:hover {{
            background: rgba(255, 255, 255, 0.2);
            transform: translateY(-50%) scale(1.1);
        }}

        .nav-btn:active {{
            transform: translateY(-50%) scale(0.95);
        }}

        .nav-btn.prev {{
            left: 20px;
        }}

        .nav-btn.next {{
            right: 20px;
        }}

        .nav-btn:disabled {{
            opacity: 0.3;
            cursor: not-allowed;
        }}

        .nav-btn:disabled:hover {{
            transform: translateY(-50%);
            background: rgba(255, 255, 255, 0.1);
        }}

        /* Top controls */
        .lightbox-controls {{
            position: fixed;
            top: 20px;
            right: 20px;
            display: flex;
            gap: 10px;
            z-index: 1001;
        }}

        .lightbox-btn {{
            background: rgba(255, 255, 255, 0.1);
            border: none;
            padding: 12px 20px;
            cursor: pointer;
            font-size: 0.9rem;
            color: white;
            border-radius: 6px;
            transition: all 0.2s ease;
            backdrop-filter: blur(10px);
            font-weight: 500;
        }}

        .lightbox-btn:hover {{
            background: rgba(255, 255, 255, 0.2);
        }}

        .close-btn {{
            position: fixed;
            top: 20px;
            left: 20px;
            background: rgba(255, 255, 255, 0.1);
            border: none;
            width: 44px;
            height: 44px;
            cursor: pointer;
            font-size: 1.5rem;
            color: white;
            border-radius: 6px;
            z-index: 1001;
            transition: all 0.2s ease;
            backdrop-filter: blur(10px);
            display: flex;
            align-items: center;
            justify-content: center;
        }}

        .close-btn:hover {{
            background: rgba(255, 255, 255, 0.2);
        }}

        /* Image counter */
        .image-counter {{
            position: fixed;
            bottom: 30px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(255, 255, 255, 0.1);
            padding: 8px 20px;
            border-radius: 20px;
            color: white;
            font-size: 0.9rem;
            z-index: 1001;
            backdrop-filter: blur(10px);
            font-weight: 500;
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

            /* Hide navigation arrows on mobile - use swipe instead */
            .nav-btn {{
                display: none;
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
        <button class="nav-btn prev" id="prev-btn" onclick="navigateImage(-1)">‹</button>
        <button class="nav-btn next" id="next-btn" onclick="navigateImage(1)">›</button>
        <div class="lightbox-controls">
            <button class="lightbox-btn" onclick="downloadImage()">Download</button>
        </div>
        <div class="image-counter" id="image-counter">1 / 1</div>
        <div class="lightbox-content">
            <img class="lightbox-image" id="lightbox-img" src="" alt="">
        </div>
    </div>

    <script>
        const albumId = '{album_id}';
        const images = {images_json};
        let currentImageIndex = 0;

        // Track which images have which tiers loaded
        const loadedTiers = {{}};

        // Progressive enhancement: upgrade thumbnails to previews in the gallery
        document.addEventListener('DOMContentLoaded', () => {{
            images.forEach((image, index) => {{
                if (image.preview_url) {{
                    const thumbImg = document.querySelector(`#gallery .bento-item:nth-child(${{index + 1}}) img`);
                    if (thumbImg) {{
                        const previewImg = new Image();
                        previewImg.onload = () => {{
                            thumbImg.src = previewImg.src;
                            if (!loadedTiers[index]) loadedTiers[index] = {{}};
                            loadedTiers[index].preview = true;
                        }};
                        previewImg.src = image.preview_url;
                    }}
                }}
            }});
        }});

        function openLightbox(index) {{
            currentImageIndex = index;
            showImage(index);
            document.getElementById('lightbox').classList.add('active');
            updateNavButtons();
            preloadAdjacentImages();
        }}

        function showImage(index) {{
            const image = images[index];
            const lightboxImg = document.getElementById('lightbox-img');
            const counter = document.getElementById('image-counter');

            // Use best available tier: preview if already loaded, otherwise thumbnail
            const tiers = loadedTiers[index] || {{}};
            let initialSrc = image.thumbnail_url || `/api/album/${{albumId}}/image/${{image.thumbnail_path}}`;

            if (tiers.preview && image.preview_url) {{
                initialSrc = image.preview_url;
            }}

            // Show best available tier immediately
            lightboxImg.src = initialSrc;

            // Update counter
            counter.textContent = `${{index + 1}} / ${{images.length}}`;

            // Load original in background and swap when ready
            const originalUrl = image.original_url || `/api/album/${{albumId}}/image/${{image.original_path}}`;
            const fullImg = new Image();
            fullImg.onload = () => {{
                lightboxImg.src = fullImg.src;
                if (!loadedTiers[index]) loadedTiers[index] = {{}};
                loadedTiers[index].original = true;
            }};
            fullImg.src = originalUrl;
        }}

        function navigateImage(direction) {{
            const newIndex = currentImageIndex + direction;
            if (newIndex >= 0 && newIndex < images.length) {{
                currentImageIndex = newIndex;
                showImage(newIndex);
                updateNavButtons();
                preloadAdjacentImages();
            }}
        }}

        function updateNavButtons() {{
            const prevBtn = document.getElementById('prev-btn');
            const nextBtn = document.getElementById('next-btn');
            prevBtn.disabled = currentImageIndex === 0;
            nextBtn.disabled = currentImageIndex === images.length - 1;
        }}

        function preloadAdjacentImages() {{
            // Preload next and previous originals
            [-1, 1].forEach(offset => {{
                const idx = currentImageIndex + offset;
                if (idx >= 0 && idx < images.length) {{
                    const img = images[idx];
                    const originalUrl = img.original_url || `/api/album/${{albumId}}/image/${{img.original_path}}`;
                    const preloadImg = new Image();
                    preloadImg.onload = () => {{
                        if (!loadedTiers[idx]) loadedTiers[idx] = {{}};
                        loadedTiers[idx].original = true;
                    }};
                    preloadImg.src = originalUrl;
                }}
            }});
        }}

        function closeLightbox() {{
            document.getElementById('lightbox').classList.remove('active');
        }}

        function downloadImage() {{
            const image = images[currentImageIndex];
            const link = document.createElement('a');
            link.href = image.original_url || `/api/album/${{albumId}}/image/${{image.original_path}}`;
            link.download = image.original_filename;
            document.body.appendChild(link);
            link.click();
            document.body.removeChild(link);
        }}

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {{
            const lightbox = document.getElementById('lightbox');
            if (!lightbox.classList.contains('active')) return;

            if (e.key === 'Escape') {{
                closeLightbox();
            }} else if (e.key === 'ArrowLeft') {{
                navigateImage(-1);
            }} else if (e.key === 'ArrowRight') {{
                navigateImage(1);
            }}
        }});

        // Close on background click
        document.getElementById('lightbox').addEventListener('click', (e) => {{
            if (e.target.id === 'lightbox') closeLightbox();
        }});

        // Mobile swipe navigation
        let touchStartX = 0;
        let touchEndX = 0;
        const lightboxContent = document.querySelector('.lightbox-content');

        lightboxContent.addEventListener('touchstart', (e) => {{
            touchStartX = e.changedTouches[0].screenX;
        }}, false);

        lightboxContent.addEventListener('touchend', (e) => {{
            touchEndX = e.changedTouches[0].screenX;
            handleSwipe();
        }}, false);

        function handleSwipe() {{
            const swipeThreshold = 50;
            const diff = touchStartX - touchEndX;

            if (Math.abs(diff) > swipeThreshold) {{
                if (diff > 0) {{
                    // Swiped left - next image
                    navigateImage(1);
                }} else {{
                    // Swiped right - previous image
                    navigateImage(-1);
                }}
            }}
        }}
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
            let thumbnail_src = image
                .thumbnail_url
                .clone()
                .unwrap_or_else(|| {
                    // Fallback to proxigned URL if presigned URL not available
                    format!("/api/album/{}/image/{}", album_id, image.thumbnail_path)
                });

            format!(
                r#"<div class="bento-item" onclick="openLightbox({index})">
                <img src="{thumbnail_src}" alt="{filename}" loading="lazy">
            </div>"#,
                index = index,
                thumbnail_src = html_escape(&thumbnail_src),
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

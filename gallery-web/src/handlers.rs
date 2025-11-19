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

    let mut manifest: AlbumManifest = match serde_json::from_str(&manifest_json) {
        Ok(m) => m,
        Err(_) => return Html(generate_404_html()),
    };

    // Generate presigned URLs for direct S3 access (valid for 7 days to match object expiration)
    let expires_in = std::time::Duration::from_secs(7 * 24 * 3600);
    for image in &mut manifest.images {
        let thumbnail_key = format!("{album_id}/{}", image.thumbnail_path);
        let preview_key = format!("{album_id}/{}", image.preview_path);
        let original_key = format!("{album_id}/{}", image.original_path);

        image.thumbnail_url = state.s3.generate_presigned_url(&thumbnail_key, expires_in).await.ok();
        image.preview_url = state.s3.generate_presigned_url(&preview_key, expires_in).await.ok();
        image.original_url = state.s3.generate_presigned_url(&original_key, expires_in).await.ok();
    }

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

    // Generate presigned URLs for all images (valid for 7 days to match object expiration)
    let expires_in = std::time::Duration::from_secs(7 * 24 * 3600);
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
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
    <meta name="apple-mobile-web-app-capable" content="yes">
    <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
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
            /* Safe area insets for notched devices */
            padding-top: env(safe-area-inset-top);
        }}

        body.lightbox-open {{
            overflow: hidden;
            position: fixed;
            width: 100%;
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
            transition: opacity 0.3s ease;
        }}

        .bento-item img.loading {{
            opacity: 0.7;
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
            width: 90vw;
            height: 90vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}

        .lightbox-image {{
            width: 100%;
            height: 100%;
            object-fit: contain;
            user-select: none;
            transition: opacity 0.2s ease;
            touch-action: pan-x pan-y;
            -webkit-touch-callout: none;
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
            /* Safe area for notched devices */
            top: max(20px, env(safe-area-inset-top));
            right: max(20px, env(safe-area-inset-right));
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
            min-height: 44px;
        }}

        .lightbox-btn:hover {{
            background: rgba(255, 255, 255, 0.2);
        }}

        .lightbox-btn:active {{
            transform: scale(0.95);
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
            /* Safe area for notched devices */
            top: max(20px, env(safe-area-inset-top));
            left: max(20px, env(safe-area-inset-left));
        }}

        .close-btn:hover {{
            background: rgba(255, 255, 255, 0.2);
        }}

        .close-btn:active {{
            transform: scale(0.95);
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
            /* Safe area for devices with bottom insets */
            bottom: max(30px, env(safe-area-inset-bottom));
        }}

        @media (max-width: 768px) {{
            .header h1 {{
                font-size: 2rem;
            }}

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

            /* Larger touch targets on mobile */
            .close-btn {{
                width: 48px;
                height: 48px;
            }}

            .lightbox-btn {{
                min-height: 48px;
                padding: 14px 24px;
            }}

            .image-counter {{
                font-size: 1rem;
                padding: 10px 24px;
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

        // Cache for preloaded Image objects to prevent garbage collection
        const imageCache = {{}};

        // Progressive enhancement: upgrade thumbnails to previews in the gallery
        document.addEventListener('DOMContentLoaded', () => {{
            images.forEach((image, index) => {{
                const previewUrl = image.preview_url || `/api/album/${{albumId}}/image/${{image.preview_path}}`;
                const thumbImg = document.querySelector(`img[data-index="${{index}}"]`);

                if (thumbImg && previewUrl) {{
                    const previewImg = new Image();
                    previewImg.onload = () => {{
                        // Direct swap - no flashing fade animation
                        thumbImg.src = previewImg.src;

                        if (!loadedTiers[index]) loadedTiers[index] = {{}};
                        loadedTiers[index].preview = true;
                    }};
                    previewImg.src = previewUrl;
                }}
            }});
        }});

        function openLightbox(index) {{
            currentImageIndex = index;
            showImage(index);
            document.getElementById('lightbox').classList.add('active');
            document.body.classList.add('lightbox-open');
            updateNavButtons();
            preloadAdjacentImages();
        }}

        function showImage(index) {{
            const image = images[index];
            const lightboxImg = document.getElementById('lightbox-img');
            const counter = document.getElementById('image-counter');

            const tiers = loadedTiers[index] || {{}};
            const originalUrl = image.original_url || `/api/album/${{albumId}}/image/${{image.original_path}}`;
            const previewUrl = image.preview_url || `/api/album/${{albumId}}/image/${{image.preview_path}}`;
            const thumbnailUrl = image.thumbnail_url || `/api/album/${{albumId}}/image/${{image.thumbnail_path}}`;

            // Update counter
            counter.textContent = `${{index + 1}} / ${{images.length}}`;

            // If original is already loaded, show it immediately - no re-download
            if (tiers.original) {{
                lightboxImg.style.opacity = '1';
                lightboxImg.src = originalUrl;
                return;
            }}

            // Determine best available tier to show while loading original
            let initialSrc = thumbnailUrl;
            if (tiers.preview || image.preview_url) {{
                initialSrc = previewUrl;
            }}

            // Show best available tier immediately
            lightboxImg.style.opacity = '1';
            lightboxImg.src = initialSrc;

            // If showing thumbnail and preview not loaded yet, load preview first
            if (initialSrc === thumbnailUrl && !tiers.preview && previewUrl) {{
                const previewImg = new Image();
                previewImg.onload = () => {{
                    lightboxImg.style.opacity = '0.3';
                    setTimeout(() => {{
                        lightboxImg.src = previewImg.src;
                        lightboxImg.style.opacity = '1';
                    }}, 50);
                    if (!loadedTiers[index]) loadedTiers[index] = {{}};
                    loadedTiers[index].preview = true;
                }};
                previewImg.src = previewUrl;
            }}

            // Load original in background and swap when ready
            const fullImg = new Image();
            fullImg.onload = () => {{
                // Smooth transition to full-res
                lightboxImg.style.opacity = '0.5';
                setTimeout(() => {{
                    lightboxImg.src = fullImg.src;
                    lightboxImg.style.opacity = '1';
                }}, 50);
                if (!loadedTiers[index]) loadedTiers[index] = {{}};
                loadedTiers[index].original = true;

                // Cache the image object to prevent garbage collection
                if (!imageCache[index]) imageCache[index] = {{}};
                imageCache[index].original = fullImg;
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
                    const tiers = loadedTiers[idx] || {{}};

                    // Skip if already loaded
                    if (tiers.original) return;

                    const img = images[idx];
                    const originalUrl = img.original_url || `/api/album/${{albumId}}/image/${{img.original_path}}`;
                    const preloadImg = new Image();
                    preloadImg.onload = () => {{
                        if (!loadedTiers[idx]) loadedTiers[idx] = {{}};
                        loadedTiers[idx].original = true;

                        // Store in cache to prevent garbage collection
                        if (!imageCache[idx]) imageCache[idx] = {{}};
                        imageCache[idx].original = preloadImg;
                    }};
                    preloadImg.src = originalUrl;
                }}
            }});
        }}

        function closeLightbox() {{
            document.getElementById('lightbox').classList.remove('active');
            document.body.classList.remove('lightbox-open');
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
                <img data-index="{index}" src="{thumbnail_src}" alt="{filename}" loading="lazy">
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

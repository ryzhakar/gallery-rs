# Film Gallery

A minimalist, S3-backed photo gallery system designed for film photographers. Features private link-based access, bento-style layouts, and full-resolution image viewing with grain preservation.

## Features

- **CLI Tool**: Process and upload images with automatic resizing (thumbnails, previews, originals)
- **Web Gallery**: Beautiful bento-style grid layout with lightbox viewer
- **S3-Based**: No database required - all data stored in S3-compatible storage
- **Private Links**: UUID-based album access without authentication overhead
- **Film-Friendly**: High-quality JPEG encoding to preserve analogue grain
- **Download Support**: One-click full-resolution downloads

## Architecture

```
┌─────────────┐         ┌──────────────┐         ┌─────────┐
│   CLI Tool  │────────▶│   S3 Bucket  │◀────────│  Web    │
│   (Rust)    │ uploads │  (images +   │  reads  │  App    │
│             │         │   manifests) │         │ (Axum)  │
└─────────────┘         └──────────────┘         └─────────┘
```

### S3 Structure

```
bucket/
  {album-uuid}/
    manifest.json
    thumbnails/
      {image-id}.jpg  (400px max)
    previews/
      {image-id}.jpg  (2048px max)
    originals/
      {image-id}.jpg  (full resolution)
```

## Setup

### Prerequisites

- Rust 1.75+ (install from [rustup.rs](https://rustup.rs))
- AWS credentials configured (for S3 access)
- S3-compatible bucket (AWS S3, DigitalOcean Spaces, MinIO, etc.)

### AWS/S3 Configuration

Configure your AWS credentials and region:

```bash
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export AWS_REGION="us-east-1"  # or your region
export GALLERY_BUCKET="your-bucket-name"
```

For S3-compatible services (not AWS), also set:

```bash
export AWS_ENDPOINT_URL="https://your-endpoint.com"
```

### Build

```bash
# Build everything
cargo build --release

# CLI binary will be at: target/release/gallery
# Web binary will be at: target/release/gallery-web
```

## Usage

### CLI Tool

#### Upload an Album

```bash
# Upload images from a directory
./target/release/gallery upload \
  --name "Summer 2024" \
  --bucket "my-gallery-bucket" \
  /path/to/photos/

# Upload specific files
./target/release/gallery upload \
  --name "Best Shots" \
  --bucket "my-gallery-bucket" \
  photo1.jpg photo2.jpg photo3.jpg
```

The CLI will:
1. Process each image (resize, optimize)
2. Upload thumbnails, previews, and originals to S3
3. Generate and upload a manifest
4. Output the album UUID for accessing the gallery

#### Delete an Album

```bash
./target/release/gallery delete \
  --bucket "my-gallery-bucket" \
  ALBUM-UUID-HERE
```

### Web App

#### Running Locally

```bash
export GALLERY_BUCKET="my-gallery-bucket"
export PORT=3000  # optional, defaults to 3000

./target/release/gallery-web
```

Visit: `http://localhost:3000/gallery/{album-uuid}`

#### Deploying to Coolify

1. **Create a new service** in Coolify
2. **Set as Dockerfile-based** deployment
3. **Add environment variables**:
   ```
   GALLERY_BUCKET=your-bucket-name
   AWS_ACCESS_KEY_ID=your-key
   AWS_SECRET_ACCESS_KEY=your-secret
   AWS_REGION=your-region
   PORT=3000
   ```
4. **Create Dockerfile** in the repository:
   ```dockerfile
   FROM rust:1.75 as builder
   WORKDIR /app
   COPY . .
   RUN cargo build --release --bin gallery-web

   FROM debian:bookworm-slim
   RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
   COPY --from=builder /app/target/release/gallery-web /usr/local/bin/gallery-web
   EXPOSE 3000
   CMD ["gallery-web"]
   ```
5. Deploy!

### macOS Shortcut Integration

To create a macOS shortcut for easy uploads:

1. Build and install the CLI:
   ```bash
   cargo build --release
   sudo cp target/release/gallery /usr/local/bin/
   ```

2. Open **Shortcuts.app** on macOS

3. Create a new shortcut with:
   - **Receive**: Images from Share Sheet
   - **Run Shell Script**:
     ```bash
     export GALLERY_BUCKET="your-bucket-name"
     export AWS_ACCESS_KEY_ID="your-key"
     export AWS_SECRET_ACCESS_KEY="your-secret"

     /usr/local/bin/gallery upload \
       --name "$(date +%Y-%m-%d)" \
       --bucket "$GALLERY_BUCKET" \
       "$@"
     ```

4. Now you can share images directly from Photos.app to create galleries!

## Configuration

### Environment Variables

#### CLI
- `GALLERY_BUCKET`: S3 bucket name (required)
- `AWS_ACCESS_KEY_ID`: AWS access key (required)
- `AWS_SECRET_ACCESS_KEY`: AWS secret key (required)
- `AWS_REGION`: AWS region (default: us-east-1)
- `AWS_ENDPOINT_URL`: Custom S3 endpoint for non-AWS services

#### Web App
- `GALLERY_BUCKET`: S3 bucket name (required)
- `AWS_ACCESS_KEY_ID`: AWS access key (required)
- `AWS_SECRET_ACCESS_KEY`: AWS secret key (required)
- `AWS_REGION`: AWS region (default: us-east-1)
- `AWS_ENDPOINT_URL`: Custom S3 endpoint
- `PORT`: Server port (default: 3000)

### Image Processing Settings

Edit `gallery-cli/src/image_processor.rs` to adjust:
- `THUMBNAIL_SIZE`: Default 400px (for grid)
- `PREVIEW_SIZE`: Default 2048px (for lightbox initial load)
- `JPEG_QUALITY`: Default 92 (high quality for film grain)

## Development

### Project Structure

```
gallery-rs/
├── gallery-core/      # Shared library (S3, manifests)
├── gallery-cli/       # CLI tool for uploads
├── gallery-web/       # Web server (Axum)
└── Cargo.toml         # Workspace configuration
```

### Running Tests

```bash
cargo test --workspace
```

### Formatting & Linting

```bash
cargo fmt --all
cargo clippy --workspace
```

## License

MIT License - see LICENSE file

## Contributing

PRs welcome! Please ensure:
- Code is formatted (`cargo fmt`)
- No clippy warnings (`cargo clippy`)
- Tests pass (`cargo test`)

---

Built with Rust 🦀 for film photographers 📷

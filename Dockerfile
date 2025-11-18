# Build stage
FROM rust:1.88 as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml ./
COPY gallery-core ./gallery-core
COPY gallery-cli ./gallery-cli
COPY gallery-web ./gallery-web

# Build the web app
RUN cargo build --release --bin gallery-web

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/gallery-web /usr/local/bin/gallery-web

# Expose port
EXPOSE 3000

# Run the web server
CMD ["gallery-web"]

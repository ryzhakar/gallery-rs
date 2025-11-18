mod commands;
mod image_processor;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "gallery")]
#[command(about = "Film gallery CLI tool for S3-based photo management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Upload images to create a new album
    Upload {
        /// Directory or files to upload
        #[arg(required = true)]
        paths: Vec<String>,

        /// Album name
        #[arg(short, long)]
        name: String,

        /// S3 bucket name
        #[arg(short, long, env = "GALLERY_BUCKET")]
        bucket: String,
    },

    /// Delete an album
    Delete {
        /// Album ID to delete
        album_id: String,

        /// S3 bucket name
        #[arg(short, long, env = "GALLERY_BUCKET")]
        bucket: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gallery_cli=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Upload { paths, name, bucket } => {
            commands::upload::execute(paths, name, bucket).await?;
        }
        Commands::Delete { album_id, bucket } => {
            commands::delete::execute(album_id, bucket).await?;
        }
    }

    Ok(())
}

//! rust-imager executable.

mod app;
mod wizard;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rust_imager_core::plan::{Compression, VerificationLevel};

#[derive(Debug, Parser)]
#[command(version, about = "Safely shrink and image external SBC eMMC devices")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List safe external USB /dev/sdX candidates as JSON.
    List,
    /// Run non-interactively after exact model confirmation.
    Image {
        /// Whole external disk, for example /dev/sda.
        #[arg(long)]
        device: String,
        /// Local output image path.
        #[arg(long)]
        output: PathBuf,
        /// Exact model string displayed by `list`.
        #[arg(long)]
        confirm_model: String,
        /// Compression format.
        #[arg(long, value_enum, default_value_t = CompressionArg::Zstd)]
        compression: CompressionArg,
        /// Encoder level.
        #[arg(long, default_value_t = 3)]
        level: u32,
        /// Verification policy.
        #[arg(long, value_enum, default_value_t = VerifyArg::Decode)]
        verify: VerifyArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompressionArg {
    Zstd,
    Xz,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum VerifyArg {
    None,
    Hash,
    Decode,
    Reread,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    app::require_linux_root()?;
    match cli.command {
        Some(Command::List) => {
            println!("{}", serde_json::to_string_pretty(&app::discover(None)?)?);
        }
        Some(Command::Image {
            device,
            output,
            confirm_model,
            compression,
            level,
            verify,
        }) => {
            let compression = match compression {
                CompressionArg::Zstd => Compression::Zstandard {
                    level: i32::try_from(level)?,
                },
                CompressionArg::Xz => Compression::Xz { level },
            };
            app::run_image(&app::ImageRequest {
                device,
                output,
                confirm_model,
                compression,
                verification: match verify {
                    VerifyArg::None => VerificationLevel::None,
                    VerifyArg::Hash => VerificationLevel::StreamingHash,
                    VerifyArg::Decode => VerificationLevel::Decode,
                    VerifyArg::Reread => VerificationLevel::SourceReread,
                },
            })?;
        }
        None => {
            let devices = app::discover(None)?;
            app::run_image(&wizard::run(devices)?)?;
        }
    }
    Ok(())
}

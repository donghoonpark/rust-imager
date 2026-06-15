//! rust-imager executable.

mod app;
mod flash;
mod wizard;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rust_imager_core::flash::{PostVerify, PreVerify};
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
    /// Flash a raw or compressed image to a confirmed external USB disk.
    Flash {
        /// Raw `.img`, `.img.zst`, or `.img.xz` input.
        #[arg(long)]
        image: PathBuf,
        /// Whole external target disk, for example /dev/sda.
        #[arg(long)]
        device: String,
        /// Exact target model string displayed by `list`.
        #[arg(long)]
        confirm_model: String,
        /// Input verification before writing.
        #[arg(long, value_enum, default_value_t = PreVerifyArg::Basic)]
        pre_verify: PreVerifyArg,
        /// Optional target reread after writing.
        #[arg(long, value_enum, default_value_t = PostVerifyArg::None)]
        post_verify: PostVerifyArg,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PreVerifyArg {
    None,
    Basic,
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PostVerifyArg {
    None,
    Full,
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
        Some(Command::Flash {
            image,
            device,
            confirm_model,
            pre_verify,
            post_verify,
        }) => {
            flash::run(&flash::FlashRequest {
                image,
                device,
                confirm_model,
                pre_verify: match pre_verify {
                    PreVerifyArg::None => PreVerify::None,
                    PreVerifyArg::Basic => PreVerify::Basic,
                    PreVerifyArg::Full => PreVerify::Full,
                },
                post_verify: match post_verify {
                    PostVerifyArg::None => PostVerify::None,
                    PostVerifyArg::Full => PostVerify::Full,
                },
            })?;
        }
        None => {
            let devices = app::discover(None)?;
            match wizard::run(devices)? {
                wizard::WizardRequest::Image(request) => wizard::run_operation(&request)?,
                wizard::WizardRequest::Flash(request) => wizard::run_flash_operation(&request)?,
            }
        }
    }
    Ok(())
}

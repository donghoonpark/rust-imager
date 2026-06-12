//! rust-imager executable.

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about = "Safely shrink and image external SBC eMMC devices")]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

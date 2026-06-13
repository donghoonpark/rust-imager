//! Privileged block and filesystem failure injection.

use std::env;
use std::path::{Path, PathBuf};

use rust_imager_core::plan::Compression;
use rust_imager_pipeline::extract::{ExtractOptions, extract_path, partial_path};

#[test]
#[ignore = "requires Linux device-mapper or a constrained filesystem"]
fn retains_partial_output_after_injected_io_failure() {
    let source = PathBuf::from(env::var("RUST_IMAGER_FAILURE_SOURCE").expect("failure source"));
    let output = PathBuf::from(env::var("RUST_IMAGER_FAILURE_OUTPUT").expect("failure output"));
    let bytes = env::var("RUST_IMAGER_FAILURE_BYTES")
        .expect("failure bytes")
        .parse()
        .expect("numeric failure bytes");
    let result = extract_path(
        Path::new(&source),
        &output,
        &ExtractOptions {
            bytes,
            compression: Compression::Zstandard { level: 1 },
            raw_hash: true,
            buffer_bytes: 64 * 1024,
            queue_depth: 2,
        },
        |_| {},
    );
    assert!(result.is_err(), "injected I/O failure must be reported");
    assert!(!output.exists(), "failed output must not be finalized");
    assert!(
        partial_path(&output).exists(),
        "failed extraction must retain its partial artifact"
    );
}

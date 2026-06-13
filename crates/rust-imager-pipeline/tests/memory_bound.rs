//! Reader queue memory-bound tests.

use rust_imager_core::plan::Compression;
use rust_imager_pipeline::extract::ExtractOptions;

#[test]
fn queued_capacity_depends_only_on_buffer_configuration() {
    let small_source = ExtractOptions {
        bytes: 64 * 1024 * 1024,
        compression: Compression::Zstandard { level: 3 },
        raw_hash: true,
        buffer_bytes: 1024 * 1024,
        queue_depth: 4,
    };
    let huge_source = ExtractOptions {
        bytes: 4 * 1024 * 1024 * 1024 * 1024,
        ..small_source
    };
    assert_eq!(small_source.queued_input_capacity(), Some(4 * 1024 * 1024));
    assert_eq!(
        small_source.queued_input_capacity(),
        huge_source.queued_input_capacity()
    );
}

#[test]
fn rejects_queue_capacity_overflow() {
    let options = ExtractOptions {
        bytes: 1,
        compression: Compression::Zstandard { level: 1 },
        raw_hash: false,
        buffer_bytes: usize::MAX,
        queue_depth: 2,
    };
    assert_eq!(options.queued_input_capacity(), None);
}

# Performance Validation

`rust-imager` optimizes for avoiding unused eMMC reads first, then for keeping
the remaining sequential read pipeline busy.

## Benchmark

Run:

```bash
cargo bench -p rust-imager-pipeline --bench throughput
```

The benchmark compares an uncompressed copy with Zstandard level 3 over the
same 64 MiB input. Absolute numbers from shared CI runners are informational.
Acceptance uses the compressed pipeline's throughput divided by raw-copy
throughput on the same runner.

## Memory Bound

The default pipeline has four queued 1 MiB input buffers. Reader-side queued
memory is therefore capped at approximately 4 MiB, plus one active encoder
buffer and library state. It does not scale with eMMC capacity.

## Linux Device Measurement

For USB 2.0 validation:

1. Drop unrelated workloads and record the USB topology with `lsusb -t`.
2. Measure the planned range with `dd if=/dev/sdX of=/dev/null bs=1M count=N
   iflag=direct status=progress`.
3. Run Zstandard level 3 with SHA-256 enabled over the same range.
4. Record average MiB/s, CPU utilization, maximum RSS, result size and reader
   model.

The release target is at least 95% of raw sequential throughput when CPU and
the local output filesystem are fast enough. XZ maximum compression is allowed
to be CPU-bound and is reported separately.

`O_DIRECT` is not enabled by default. USB bridges and filesystems vary in their
alignment and direct-I/O behavior; sequential buffered reads with a bounded
queue are the compatibility baseline.

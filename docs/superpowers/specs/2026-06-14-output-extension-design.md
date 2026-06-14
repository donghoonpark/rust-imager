# Automatic Output Extension Design

## Goal

Reduce TUI input friction by completing an output filename from the selected
compression format without weakening the engine's strict validation.

## Behavior

- `board` becomes `board.img.zst` for Zstandard and `board.img.xz` for XZ.
- `board.img` becomes `board.img.zst` or `board.img.xz`.
- A path already ending in the selected compression extension is unchanged.
- A path with any other extension, including the opposite compression
  extension, is unchanged and is rejected later by the existing engine
  validation.
- Empty output remains an error.
- The behavior applies to the interactive TUI only. Non-interactive CLI input
  remains explicit and strictly validated.

## Architecture

The pure TUI model normalizes the output when `Continue` is reduced on the
output screen. This keeps path behavior deterministic and directly testable.
The application engine remains the final authority and continues requiring the
extension to match the selected compression format.

## Verification

Reducer tests cover extensionless paths, `.img` paths, matching paths, XZ, and
conflicting extensions. Existing CLI validation tests ensure strict engine
behavior is preserved.

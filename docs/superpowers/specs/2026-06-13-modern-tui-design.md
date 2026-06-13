# Modern TUI Design

## Goal

Turn the functional wizard into a polished modern dashboard that communicates
device identity, destructive risk, workflow position, current work, progress,
and recovery information without weakening existing safety gates.

## Visual Direction

The interface combines a modern dashboard with an industrial storage console.
Its default palette uses cyan for identity and navigation, blue for active
work, green for completed work, amber for caution, and red only for destructive
risk or failure. Styling degrades to terminal defaults where color is
unavailable.

Wide terminals display a compact multi-line `rust-imager` ASCII wordmark.
Medium and small terminals replace it with a one-line title so operational
information always has priority over decoration.

## Layout

The wide layout contains:

1. branded header with version, selected device, and current phase;
2. horizontal workflow stepper;
3. main content area split between the active panel and contextual sidebar;
4. persistent status or error line;
5. context-aware key command bar.

The setup wizard uses the main panel for selection, confirmation, output,
verification, and immutable review. The sidebar shows source-device facts,
selected policies, and safety notices.

The operation dashboard uses the main panel for phase timeline, overall
progress, phase progress, metrics, and recent events. The sidebar keeps source
and output identity visible throughout destructive work.

## Responsive Behavior

- Wide, at least 100 columns and 28 rows: full wordmark, split body, metrics,
  and recent-event panel.
- Medium, at least 72 columns and 20 rows: one-line brand, split or stacked
  cards, reduced event count.
- Compact, at least 50 columns and 14 rows: stacked essentials, single progress
  gauge, no wordmark or event panel.
- Smaller terminals retain the existing explicit minimum-size message.

No layout may panic or clip outside the frame.

## Wizard Improvements

- A numbered stepper shows `Device`, `Confirm`, `Output`, `Verify`, and `Review`.
- Device selection uses a highlighted list with human-readable capacities and
  model/path columns.
- Confirmation shows device facts and separates ordinary destructive warning
  from the stronger unknown-layout warning.
- Output and verification screens expose available choices and current
  selection instead of showing only raw debug values.
- Review presents an immutable summary table and a prominent final mutation
  warning.
- The command bar shows only keys valid on the current screen.

Existing exact-model confirmation and pre-mutation Escape behavior remain
unchanged.

## Operation Dashboard

The operation timeline contains:

1. Inspect
2. Shrink
3. Extract
4. Verify
5. Complete

Each phase is pending, active, complete, or failed. The dashboard distinguishes
overall phase progress from byte extraction progress.

The model tracks:

- phase start time and total elapsed time;
- bytes processed and total planned bytes;
- instantaneous throughput based on recent progress samples;
- average throughput;
- estimated remaining time when enough progress exists;
- recent engine event messages;
- operation failure state.

Extraction shows percentage, processed/total bytes, speed, elapsed time, and
ETA. Preparing, mutation, and verification use indeterminate activity
indicators because their external tools do not currently report byte progress.
The UI must not invent percentages for those phases.

Compressed output size and compression ratio are excluded until the pipeline
emits trustworthy live output-byte events.

## Event Flow

The existing engine events remain the source of phase transitions. The CLI
operation loop records a timestamped UI event for each transition and forwards
byte updates to the TUI model. A periodic tick redraws spinners, elapsed time,
and ETA even when no engine event arrives.

The engine runs on a worker thread while the terminal loop redraws at a bounded
rate. Terminal drawing remains on the main thread. Completion and error results
are returned through a channel, avoiding unsafe shared terminal access.

## Error Handling

Failures keep the dashboard visible with the failed phase marked red, an
actionable wrapped error panel, output artifact state where available, and an
Enter/Escape acknowledgement prompt. Terminal restoration remains guaranteed
on every return path.

Unknown-layout warnings remain visible from confirmation through review and are
also recorded in the operation event list.

## Internal Structure

- `model.rs` owns phase, progress samples, metrics, recent events, and pure
  state transitions.
- `view.rs` remains the rendering entry point but delegates to focused modules
  for theme, layout, header, wizard panels, operation panels, and formatting.
- `wizard.rs` owns keyboard input and the asynchronous operation/render loop.
- Pure formatting and responsive-layout decisions are unit tested with
  Ratatui's `TestBackend`.

## Acceptance Criteria

- The five-step wizard and five-phase operation timeline are always clear.
- Wide terminals show branding, contextual sidebar, progress metrics, and
  recent events.
- Compact terminals preserve device identity, current phase, risk, and progress
  without panic.
- Extraction reports accurate percentage, human-readable bytes, speed, elapsed
  time, and ETA.
- Non-byte phases use honest indeterminate indicators.
- Errors remain visible until acknowledged and terminal state is restored.
- All existing safety behavior and CLI operation semantics remain unchanged.

# Console editing without new crates

The optional `i --runtime console` frontend (also used by `scripts/start.sh`)
now owns terminal input and redraws the editable row after background output.
Approval policy and mode control are optional plugins. The old 1,024-line core constraint is historical; see `docs/wasm.md`. No third-party crate, Unicode table
package, `stty` subprocess, or runtime Python dependency was added. The existing
`ctrlc` dependency also handles termination signals for orderly cleanup.

## Input

See [composer.md](composer.md) for pending clipboard images (`Ctrl-V`) and folded
long/multiline text. Images wait for Enter and allow an optional caption.

- UTF-8 scalar boundaries for insert, Backspace, Delete, Left and Right.
- Home/End or Ctrl-A/Ctrl-E; Ctrl-U clears the draft; Ctrl-K deletes its suffix.
- Up/Down recalls up to 50 submissions and restores an unsubmitted draft.
  Approval responses and `/mode` control commands are excluded from history; history is memory-only.
- Enter submits. A terminal supporting **bracketed paste** inserts pasted newlines
  without submitting them. Newlines/tabs appear as `\n`/`\t` in the editable row,
  but the actual task retains them. Other pasted control characters are removed.
- In the main console, a leading `/` opens the supported-command menu with descriptions. Prefix typing filters; Up/Down selects; Tab or Enter only fills a candidate. Press Enter again with the menu closed to execute, or Escape to dismiss it. `/mode ` completes `ask`, `auto`, `status`. Pending approval commands appear only when there is a pending request. Slash characters inside prose do not open the menu.
- `/help` and the menu use the same platform-filtered `web/commands.json` catalog as the browser. Unknown commands, multiline slash commands and invalid arguments on argumentless commands never become model tasks or steering instructions. The old raw `i` readline fallback retains Tab prefix completion, not the full popup; its help no longer advertises placeholder setup/language/session operations or a fake resume action.
- Ctrl-D on an empty draft closes input (the current task can finish); with a
  nonempty draft it deletes at the cursor. Ctrl-C aborts the current task/console.
- The interactive draft is limited to 64 KiB. Oversized pastes are rejected as a
  whole, not truncated or executed as separate commands. OS process-argument
  limits can impose a smaller submission limit, particularly on Windows.

The input viewport is one row and scrolls horizontally. Chinese/full-width
characters occupy two cells; ASCII and the supported Latin range occupy one.
Unsupported glyphs, including emoji, combining marks and format controls, are
shown as ASCII Unicode escapes **only in the editable row**; their underlying
text is retained for submission. Deletion is by Unicode scalar, not grapheme.
This is deliberately **not a complete Unicode width/grapheme implementation**;
ambiguous-width terminal settings and unusual fonts can still differ. Completed
output lines can display those glyphs normally because the terminal lays them out.

## Approval modes

Use `/mode ask` (default), `/mode auto`, or `/mode` to inspect the current mode.
Console `--yolo` (alias `--dangerously-skip-permissions`; old `--yes` removed) selects initial auto but can
be revoked with `/mode ask`. Auto is not a sandbox or a policy bypass: a custom
approval worker can still ask or deny. A pending question still needs an explicit
answer after a mode switch. Mode changes apply to subsequent requests, not
already-started actions. See [replaceable approval contract](approval.md).

## Output, fallback and cleanup

Interactive editing requires stdin, stdout and stderr to be terminals and
`TERM` not to be `dumb`. Interactive stdout/stderr are displayed together through
stderr; redirected/piped operation keeps separate streams and line-based stdin.
The optional Markdown renderer can transform stdout even without a TTY; set
`NO_COLOR=1` or `TERM=dumb` to disable Markdown rendering. The standalone core's `Task:` prompt is still system line input,
not this optional editor.

Reader threads use bounded queues. One console loop owns drawing and preserves
the draft across backend messages. UTF-8 is decoded incrementally per stream;
backend terminal control characters are escaped rather than executed. Partial
output is visible as a clipped prefix until a newline/EOF commits it. Very long
unterminated output is broken into bounded lines. Authenticated stderr approval
markers are still handled separately; visual text alone cannot grant approval.

macOS/Linux use the system termios/ioctl APIs; Windows uses console modes and
virtual-terminal input/output (a VT-capable Windows console is required). Saved
modes are restored on normal exits, errors and handled interruption/termination.
SIGKILL, process crashes and `panic=abort` cannot run cleanup; use the terminal's
reset action if an abnormal exit leaves raw mode active. Job-control suspension,
full grapheme editing and every terminal's resize/reflow behavior are not promised.

Local validation uses macOS arm64 pseudo-terminals with committed UTF-8 input,
not an automated OS IME candidate-window test. Linux/Windows execution is not
claimed from that result; Linux/macOS PTY checks and Windows builds/tests are
configured in CI. No paid model requests are needed:

```sh
cargo build --workspace --features bundled --locked
python3 scripts/check_console.py
```

The PTY check covers Chinese deletion/cursor positions, insertion, history,
paste without accidental submission, concurrent split UTF-8 output, inert
backend escape sequences, authenticated approval, live mode switching,
not bypassing a pending approval, resize, and terminal-mode
restoration on exit, Ctrl-C, SIGTERM and backend startup failure.

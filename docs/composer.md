# Pending images and folded pastes

## Web

- Paste an image in the task input: `[Image #1]` appears with a removable
  thumbnail. It is **not sent** and does not become a workspace file.
- Add a caption or Send with only images. Requests contain actual image blocks
  in OpenAI/Anthropic's respective formats. The model must support image input.
- Short single-line text stays editable. Multiline text or at least 1000 Unicode
  scalar values folds to `[Pasted Content #N ... chars]`. Full content is sent,
  preserving indentation and line breaks (CRLF/CR normalize to LF), not the marker.
- Remove pending items with `×`. Typed lookalike markers are not attachments;
  token-like text inside pasted content is not expanded recursively.
- Pending image blobs and text are saved with their owning conversation in
  IndexedDB. Wait for **Saved** before reload. Switching conversations does not
  transfer attachments. Temporary workspace drafts remain memory-only; clearing
  site data also deletes persistent drafts.
- Offline/configuration failure before a run leaves the draft intact. Failed
  requests are not automatically replayed.

Limits: 64 KiB expanded text, at most 8 PNG/JPEG/GIF/WebP images, 8 MiB each and
16 MiB total. Rust rechecks MIME/signatures before sending. Clipboard data is
never read automatically on startup or in the background.

### Queued follow-up messages

During an AI task, the input stays available. **Queue message** adds a follow-up
to this conversation, not to another tab or the in-flight request. The queue
appears above the input. Pending items can be edited, saved, or cancelled;
opening an editor pauses automatic dispatch until **Continue queue** is chosen.
Editing does not alter messages already sent. Once an item starts sending its
controls are locked; use Stop to interrupt the current request instead.

Successful tasks send pending items in order. Stop, request/tool/storage errors,
or reload pause the queue; there is no automatic replay after interruption.
Continue queue explicitly resumes. Saving edits does not itself send anything.
Drafts being typed remain separate and are not cleared by queued dispatch.
Queued images/folded text keep independent attachment snapshots; image markers
can be removed while editing a caption. Slash commands and shell commands cannot
be queued. A changed connection/model must be restored before continuing a queue
created for the old connection; API keys are not copied into queue metadata.

Persistent conversations save the queue and unfinished editor text in IndexedDB;
temporary workspaces remain memory-only. Limits: 20 pending messages per
conversation and 64 MiB of queued image blobs, in addition to per-message limits.
If saving fails, the in-memory queue remains visible, dispatch is paused, and
the interface warns to copy messages before reloading. Editing/cancelling that
in-memory copy does not imply a successful storage write.

## Console

- Use the terminal's normal text paste (Cmd-V on macOS; Ctrl-Shift-V in many
  Linux terminals). Bracketed paste does not submit embedded newlines.
  Long/multiline pastes fold using the same marker convention as Web.
- **Ctrl-V** in raw input mode explicitly captures an OS clipboard image as
  `[Image #N]`. Add a caption or press Enter with only images. Backspace/Delete
  at the marker boundary removes it. Ordinary terminal Cmd-V does not reliably
  forward binary images; Ctrl-V is the explicit image path.
- macOS uses `osascript`, Windows uses PowerShell with Windows Forms, Linux
  Wayland requires `wl-paste`. Missing helpers, denied access, non-PNG clipboard
  content and a 3-second timeout give explicit errors, not fallback polling.
- Only on Enter, images are staged in private random temporary files under
  `.i/attachments/` and passed as separate `--image` arguments. Files are removed
  when the task finishes/is discarded. Default native ZIP exports exclude this
  private input directory (explicitly selected paths can still include it).
  Originals are not overwritten; abrupt
  termination can leave a private temporary file.
- Image messages submitted while busy are queued follow-ups, not hidden steering
  or approval answers. Plain text keeps existing steering/queue behavior.
- Native-console pending images do not have restart recovery. Sent images remain
  in normal session history. Console input history does not restore image blobs.

Actual OS clipboards were not accessed during automated tests: tests use synthetic
PNG data and isolated browser profiles. OS helper integrations still need manual
validation on the corresponding desktop platforms.

**Folding is display-only, not token compression.** Full text and image content
still reach the model. Use [bounded file tools](file-tools.md) to reduce irrelevant
context and model round trips.

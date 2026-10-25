# Changelog

## Unreleased

- Interactive native startup offers optional user-local installation and PATH
  setup once, then continues into configuration and the session. Declining is
  remembered; interrupted or failed installation can be retried. Noninteractive
  and explicit-config launches do not prompt. Existing executables still require
  replacement confirmation; personal configuration and sessions are preserved.

## 0.1.1

- Browser OPFS exclusive transactions serialize their own concurrent operations;
  cancellation drains active I/O and rejects queued operations. Failed new writes
  clean up uncommitted placeholders without deleting previously committed files.
- File trees refresh after tool results, before the next model response finishes.
- Browser and optional Relay model requests use a 30-minute inactivity deadline,
  renewed by response data, instead of a three-minute total duration cap.
- API and Relay credentials have separate non-submitting HTML form owners.
- A default Rust/WASM prompt-manager plugin supports workspace-local system
  prompt editing. AI changes require confirmation even in auto mode, and apply
  on the next task. It has no workspace-file or API-configuration access.
- Native release verification accepts Windows ZIP as well as tar archives,
  rejects unsafe ZIP members, and checks the actual packaged version.

Native archives record their host architecture in BUILD.json; they are not
universal binaries. macOS archives are not Developer-ID signed or notarized.
Browser-extension ZIPs are unpacked/source distributions, not store-signed
installers. No model credentials are included.

The earlier v0.1.0 tag is retained unchanged. Its Windows archive verification
failed before publication; 0.1.1 is the corrected release.

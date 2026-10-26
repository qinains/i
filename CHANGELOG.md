# Changelog

## Unreleased

- Mirror dropdown arrows and gutters in Arabic layouts; audit website and
  workspace controls across themes and improve compact disclosure hit areas.
  Give the website language selector consistent inset arrows and label space.
- Keep filled website buttons readable in light and dark themes, including
  navigation, hover and keyboard-focus states; check their contrast in browsers.
- Align the website, browser workspace and native locale registry across 17
  languages, with complete UI catalogs, build-time key/placeholder validation,
  Arabic RTL layout, region-aware defaults and persistent language selection.
  Bundle native setup translations so they work outside the source directory.
- Refine multilingual homepage copy, clarify local storage versus model API
  requests, and preserve language preferences when switching model connections.
- Make app updates wait up to 30 seconds for saved, idle pages; distinguish busy,
  unresponsive and save-failed pages, reject late acknowledgements, and retain
  the shorter safety lease when updating older app pages. Localize update status
  in all UI languages and show actionable notices on mobile.
- Use the input's monospace font for slash-command labels, and hide the update
  action when there is no waiting version after first installation.
- Replace the optional Node relay with a standalone `i-relay` executable, retaining
  protocol-native JSON/SSE, authentication, public DNS pinning, quotas, disconnect
  cancellation and inactivity deadlines. Caddy serves static assets separately.
- Lead homepage and repository introductions with user tasks and quick start;
  keep implementation details in developer references. Add a themed, multilingual
  Product Hunt link card without third-party image requests.
- Use checksum-locked crates.io globset 0.4.20 instead of a local feature patch;
  retain upstream notices without shipping a separate source copy.
- Add a three-step connection guide with official API-key links and URL/model
  examples. Explain model billing, CORS and mismatched region/plan credentials.
- Make the fullscreen preview exit control draggable with pointer capture,
  viewport bounds, mobile touch support and keyboard movement.
- Highlight local-storage, model-request, plugin, preview and browser-extension
  security boundaries on the multilingual homepage and in the security policy.
- Fullscreen browser previews fill the viewport without workspace borders,
  padding or toolbar rows. A floating exit control restores the previous layout;
  host-page Escape also exits. Preview sandbox permissions remain unchanged.

## 0.1.2

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

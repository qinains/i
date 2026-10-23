# Security policy

Do not open public issues containing live credentials, personal conversations,
private workspace files, exploitable production details, or SSH keys.

Use GitHub's private vulnerability reporting when enabled for
https://github.com/qinains/i. If unavailable, open a minimal issue requesting a
private contact channel without including the sensitive details. No response-time
or supported-version guarantee is currently offered.

Provide affected versions, attacker-controlled input, trust boundary, impact,
minimal synthetic reproduction, and expected/observed behavior. Distinguish a
static concern from an executed reproduction. Redact all credentials.

## Boundaries

- Native worker plugins are trusted processes with the user's OS privileges.
- Browser WASM plugins execute in dedicated workers with bounded host capabilities.
- CORS helper extensions are opt-in browser permissions, not a server proxy or
  an authentication bypass. Non-successful preflights remain failures.
- Browser configuration saved with Remember is readable by scripts on the same
  origin. Avoid storing keys on shared devices. No site can protect its own
  localStorage secrets from same-origin script compromise.
- OPFS isolation is cooperative reader/writer locking, not multi-file rollback,
  protection from storage eviction, or a replacement for backups.
- Preview execution is separate from plugin execution. Only trust generated HTML
  scripts after reviewing them. A snapshot preview is not a full web server.

Before making a repository public, scan both exported files and the exact Git
history to be pushed. Removing a secret from the current tree does not remove it
from history. Rotate any credential discovered in previously shared artifacts.

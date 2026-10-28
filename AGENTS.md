# Maintainer workflow

- After changing executable code, run the relevant regression tests and build
  the bundled `i` binary before reporting completion.
- The maintainer wants each verified local change installed into the `i` command
  already selected by PATH (`command -v i`). This development machine has both
  `$HOME/.cargo/bin/i` and `$HOME/.local/bin/i`; shell startup can change their
  order, so keep both installations in sync. Use `i install --bin-dir ... --replace` so replacement is
  atomic and the previous executable is backed up. Do not edit shell profiles
  just to replace an existing installation.
- Build with `cargo build --locked --profile min-size --features bundled --bin i`.
  Then install from `target/min-size/i` and verify the installed executable's
  SHA-256 matches the build. Never install after a failed build or test.
- Native connection helpers are separate executables. After changing them,
  package, verify and install the local build through `i plugins install`, enable
  it, and reconnect the active managed connection without reauthorizing accounts
  or sending a real model request. Explicitly report services that need restarting.
- Local installation does not authorize an automatic public release or website
  deployment on every edit. Publish only when requested; never overwrite an
  existing release/tag or publish credentials, account stores, pairing links,
  temporary test output, or development packages.
- Production website: `https://i.lninl.com`, deployed by GitHub Pages from `main`.
  Build and deployment checks are documented in `docs/site-deployment.md`.
  Verify the live assets after deployment; do not clear users' website storage.

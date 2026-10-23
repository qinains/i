---
name: rust-checks
description: Check Rust changes with formatting, tests, clippy and the core line budget.
---

# Rust checks

Use when changing Rust code in this harness. Read project instructions first.

1. Run `cargo fmt --all --check`.
2. Run `cargo test --workspace --locked`.
3. Run `cargo clippy --workspace --locked --all-targets -- -D warnings`.
4. Confirm the production Rust source under `src/` totals no more than 1024 lines.
5. Report actual outcomes, distinguishing local mock tests from live API or Windows validation.

Commands require the normal tool approval flow; this document does not authorize execution.

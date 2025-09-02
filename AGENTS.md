# Repository Guidelines

## Project Structure & Modules
- Root crate: desktop app in `src/` (Iced GUI, charts, layout, widgets).
- Workspace crates: `exchange/` (REST/WebSocket adapters) and `data/` (persistence, assets, audio).
- Assets: `assets/`; Packaging helpers: `scripts/`; Web entry: `index.html` (WASM via Trunk).
- CI: `.github/workflows/format.yml` (rustfmt) and `lint.yml` (clippy).

## Build, Test, and Dev Commands
- Build desktop: `cargo build --release`
- Run desktop: `cargo run --release`
- Lint (CI parity): `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Format: `cargo fmt --all`
- Test: `cargo test --workspace -- --nocapture`
- Web (optional): `rustup target add wasm32-unknown-unknown && trunk serve` (serves `index.html`).
- Packaging: Linux `scripts/package-linux.sh`; macOS `scripts/build-macos.sh`; Windows `scripts/build-windows.sh`.

## Coding Style & Naming
- Rust 2024 edition; `rustfmt` max width 100 (`rustfmt.toml`).
- Keep clippy clean; thresholds in `clippy.toml` are set for large enums/args.
- Naming: modules/functions `snake_case`, types/traits `UpperCamelCase`, constants `SCREAMING_SNAKE_CASE`.
- Prefer small, focused modules under `src/chart/*`, `src/modal/*`, `src/screen/*`, `src/widget/*`.

## Testing Guidelines
- Framework: `#[test]` and `#[tokio::test]` (async). Example: `exchange/src/adapter/hyperliquid.rs`.
- Scope: add unit tests for parsers, adapters, and chart transforms; avoid network in unit tests when possible.
- Command: `cargo test --workspace`. Some adapter tests may touch network; run locally with `-- --nocapture` for logs.

## Commit & Pull Requests
- Commits: follow Conventional Commits where possible (e.g., `feat:`, `fix:`, `chore:`). Keep messages imperative and scoped.
- PRs: include a clear description, linked issues (`#123`), screenshots for UI changes, and reproduction steps.
- Requirements: code compiles, `cargo fmt` and `cargo clippy` pass, and no new warnings. Note any platform-specific changes (Linux ALSA, etc.).

## Security & Configuration Tips
- No secrets checked in; exchange access uses public endpoints only. Avoid adding keys or tokens.
- Platform deps: Linux may need `libasound2-dev` for audio (see README). For web builds, ensure Trunk is installed.

# Web Enablement Plan

This plan outlines the concrete steps to make Flowsurface functionally usable in the web browser (WASM) beyond the current compile-only stubs.

## Current Gaps
- Native-only exchange connectors (WebSocket + REST) are not available on WASM.
- File-based persistence, packaging scripts, and multi-window behavior are not web-compatible.
- Audio and some settings are no-op on web.

## Implementation Plan
1) Platform split for exchange connectors
- Add wasm adapters in `exchange/src/wasm/{binance,bybit,hyperliquid}.rs` using `web_sys::WebSocket` and `wasm_bindgen_futures::spawn_local`.
- Keep native adapters in `exchange/src/adapter/{binance,bybit,hyperliquid}.rs`.
- In `exchange/src/adapter.rs` expose a small platform facade that delegates to native/wasm implementations via `cfg(target_arch)`. Keep shared types stable.

2) HTTP on WASM (REST)
- Use `reqwest` with its wasm client; disable TLS features under WASM.
- Exchange crate Cargo.toml (conceptual):
  - `[target.wasm32-unknown-unknown.dependencies] reqwest = { version = "0.12", default-features = false, features = ["json", "wasm"] }`
  - Gate native: `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }`
- If CORS blocks REST, add a thin proxy (see 6).

3) WebSocket clients
- Implement WS streams with `web_sys::WebSocket` per exchange. Map messages to existing `Event` and `StreamKind`.
- Handle reconnect/backoff in JS task loops (spawn_local + channels).

4) Persistence & historical data
- Replace file I/O caching with IndexedDB (e.g., `idb`/`gloo-storage`) keyed by `SerTicker` and timeframe.
- Keep layout/theme state in `localStorage` (already supported).
- For initial release, disable large historical backfills on web; prioritize live streams + small on-demand ranges.

5) Audio (WebAudio)
- Implement `data::audio` WASM path using `web_sys::AudioContext`, decode WAV assets from `/assets`, cache buffers, and schedule playback.

6) CORS/Proxy Strategy
- Add optional proxy service (tiny Axum/Express) that exposes REST routes without CORS issues.
- Make proxy URL configurable via `FLOWSURFACE_PROXY_URL` (read at runtime for WASM).

7) UI/Behavior adjustments
- Multi-window/popup panes: disable on web (single-canvas layout only). Guard with `#[cfg(target_arch = "wasm32")]`.
- File dialogs and “Open data folder”: hide or stub with toasts.

8) Build & CI
- Add `Trunk.toml` (public URL, dist dir). Example dev: `trunk serve` | prod: `trunk build --release`.
- GitHub Actions: build WASM artifacts and optionally deploy `dist/` to GitHub Pages.

## Acceptance Criteria
- `trunk serve` renders dashboard with working sidebar and charts.
- Live kline stream works in browser for at least one exchange without proxy.
- No runtime panics; console shows useful logs.
- Layout/theme persists across reloads; audio cues play on trades.

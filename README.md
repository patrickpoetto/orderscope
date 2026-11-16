# OrderScope

A live order-flow and cumulative volume delta (CVD) desktop application for
market microstructure analysis, written in Rust. OrderScope ingests
real-time trade data from Finnhub (WebSocket + REST) and renders order-flow
footprint charts, CVD curves, and derived flow analytics in a native GUI
built with [`iced`](https://github.com/iced-rs/iced).

> **Status:** work in progress. Core data plumbing, CVD modeling, and the
> first chart panes are functional; UI polish and additional analytics are
> ongoing.

## What it does

- Connects to Finnhub's trade WebSocket feed and REST endpoints to stream
  live trade prints for a given symbol.
- Classifies trades (buy/sell side, aggressor) and aggregates them into
  footprint bars, grouped volume, and time series.
- Computes cumulative volume delta (CVD) and related flow statistics
  (e.g. VPIN-style imbalance, dark-pool/institutional-flow heuristics).
- Renders everything in a native desktop GUI with a footprint/CVD pane and
  an "insider flow" pane, using a custom dark theme.

## Architecture

The project is a Cargo workspace split into focused crates, with a thin
binary crate on top for the GUI:

```
orderscope/
├── core/        # shared primitives: Trade, Event, Price/Qty/Symbol units
├── ingest/      # Finnhub WebSocket + REST ingestion (orderscope-ingest)
├── model/       # domain models: CVD time series, footprint bars, time series
├── analytics/   # derived analytics: classifier, CVD stats, VPIN, dark pool /
│                #   institutional flow detection
└── src/         # the `orderscope` binary: iced GUI (charts, panes, theme)
```

- **`core`** — foundational types shared across the workspace (`Trade`,
  `Event`, `Side`, `Price`, `Qty`, `Symbol`, timestamps).
- **`ingest`** — connects to Finnhub over `tokio-tungstenite` (WebSocket)
  and `reqwest` (REST), exposing a stream of normalized trade events.
  Includes runnable examples (`print_trades`, `print_cvd`).
- **`model`** — builds higher-level structures on top of raw trades:
  CVD time series, footprint bars, grouped volume, classified trades.
- **`analytics`** — derived signal computation: trade classification,
  CVD statistics, VPIN-style toxicity/imbalance metrics, dark-pool and
  institutional-flow heuristics.
- **`src`** (binary crate `orderscope`) — the desktop GUI, built with
  `iced`: chart rendering (`src/chart`), UI panes (`src/pane`), and theming
  (`src/theme.rs`).

## Tech stack

- **Rust** (2021 edition), Cargo workspace with 4 library crates + 1 binary
- **[`iced`](https://github.com/iced-rs/iced)** — native, cross-platform GUI
  framework (canvas-based chart rendering, `tokio` integration)
- **`tokio`** — async runtime
- **`tokio-tungstenite`** — WebSocket client for the Finnhub live trade feed
- **`reqwest`** — HTTP client for Finnhub REST endpoints
- **`serde` / `serde_json`** — data (de)serialization
- **`dotenvy`** — environment/config loading
- **`chrono`** — timestamp handling

## Building and running

Requires a recent stable Rust toolchain (`rustup`/`cargo`).

```bash
cargo build
```

The app needs a Finnhub API key, provided via the `FINNHUB_API_KEY`
environment variable (a free key can be obtained from
[finnhub.io](https://finnhub.io)):

```bash
export FINNHUB_API_KEY=your_key_here
cargo run
```

Alternatively, drop a `.env` file with `FINNHUB_API_KEY=your_key_here` in
the project root — it is loaded automatically at startup via `dotenvy`.

Runnable ingestion examples (useful for inspecting the raw data pipeline
without the GUI) live under `ingest/examples/`:

```bash
cargo run -p orderscope-ingest --example print_trades
cargo run -p orderscope-ingest --example print_cvd
```

## License

MIT — see [LICENSE](LICENSE).

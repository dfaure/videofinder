# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

VideoFinder is a cross-platform (desktop + Android) Rust application using Slint for UI. It searches a remote SQLite database of films/series and displays results with metadata and images.

## Build Commands

```bash
cargo build                  # Desktop debug build
cargo run                    # Run desktop app
cargo build --release        # Release build

cargo fmt                    # Format code
cargo clippy -- -D warnings  # Lint (all warnings are errors)
```

Pre-commit hooks enforce `cargo fmt` and `cargo clippy` on every commit.

Android builds target `aarch64-linux-android` and use Gradle (`./gradlew build` from root).

## Architecture

**UI layer** (`ui/*.slint`): Declarative Slint UI compiled at build time via `build.rs`. Uses `fluent-light` style. `app-window.slint` is the main search interface; `details-window.slint` is a popup for film details.

**Application core** (`src/lib.rs`): `App` struct holds UI handle and shared state via `Rc<RefCell<>>`. `videofinder_main()` is the entry point. Wires up Slint callbacks for search, item clicks, database download, and image display.

**Data access** (`src/sqlsearch.rs`): SQLite queries using `rusqlite`. `sqlite_search()` does full-text search across Tape/Film/Actor tables. `sqlite_get_record()` fetches detailed film info with JOINs.

**Downloads** (`src/download.rs`): Async HTTP downloads (database file, filelist, images) using `reqwest` with progress callbacks. Platform-aware storage paths.

**Image handling** (`src/image_handling.rs`): Converts database image paths to HTTP URLs and downloads images asynchronously.

**Domain enums** (`src/enums.rs`): `FilmType` and `SupportType` with display helpers.

**Binary entry** (`src/bin/main.rs`): Sets up logging (file-based on Android via `flexi_logger`, stderr on desktop) and calls `videofinder_main()`.

## Key Design Notes

- Library compiles as both `cdylib` (Android native lib) and `rlib` (desktop binary). The `with-binary` feature (default) enables the desktop binary.
- Async uses `async_compat::Compat` to bridge Slint's event loop with tokio-based async.
- Part of a larger Slint workspace; `slint` and `slint-build` come from workspace dependencies.
- No test suite exists.

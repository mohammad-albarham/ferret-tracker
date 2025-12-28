[![Built With Ratatui](https://img.shields.io/badge/Built_With_Ratatui-000?logo=ratatui&logoColor=fff)](https://ratatui.rs/)

# Ferret Tracker

<img src="assets/ferret-logo.png" alt="Ferret Logo" width="200" />

A lightweight, real-time file tracker with an interactive terminal UI for monitoring watched directories. Ferret maintains a persistent SQLite ledger of all detected files, making it easy to audit and browse file activity.

## Overview

Ferret is designed for users who need to track files appearing in specific directories in real-time. Whether monitoring download folders, artifact outputs, or project directories, Ferret provides an intuitive interface with multiple view modes, filtering, and persistent storage.

## Features

- **Real-time File Monitoring** — Native OS file system notifications (inotify/FSEvents)
- **Persistent Database** — SQLite ledger stored locally for historical tracking
- **Interactive TUI** — Navigate, search, filter, and inspect files
- **Multiple View Modes** — Flat (chronological), Grouped (by folder), and Tree (nested hierarchy)
- **Smart File Classification** — Automatically categorizes files by type
- **Flexible Configuration** — TOML-based config with per-directory ignore patterns
- **Cross-platform** — Supports Linux, macOS, and Windows

## Quick Start

### Installation

From crates.io (once published):
```bash
cargo install ferret-tracker
```

From source:
```bash
git clone https://github.com/mohammad-albarham/Ferret.git
cd Ferret
cargo build --release
./target/release/ferret-tracker watch
```

### Basic Usage

```bash
# Start the watcher with interactive TUI
cargo run -- watch

# Watch specific directories
cargo run -- watch --watch ~/Downloads --watch ~/Desktop

# Run headless (no TUI, just database population)
cargo run -- watch --headless
```

While running, create files in watched directories to see them appear in the interface.

## User Interface

### View Modes

Press `Tab` to cycle between three view modes:

| Mode | Description |
|------|-------------|
| **Flat** | Chronological list of recent file events |
| **Grouped** | Files organized under folder headers |
| **Tree** | Nested folder hierarchy with expand/collapse |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Cycle view mode |
| `↑` / `↓` or `k` / `j` | Move selection up/down |
| `←` / `→` or `h` / `l` | Collapse/expand (Tree view) |
| `Space` | Toggle expand/collapse |
| `e` / `E` | Expand all / Collapse all (Tree view) |
| `Home` / `End` | Jump to start/end of list |
| `PgUp` / `PgDn` | Page up/down |
| `Enter` | View file details |
| `f` | Open filter menu |
| `/` | Search by path |
| `o` | Open file with default program |
| `?` | Show help overlay |
| `q` / `Esc` | Quit or close overlay |

## Configuration

Configuration file location:
- **Linux/macOS**: `~/.config/ferret/config.toml`
- **Windows**: `%APPDATA%\ferret\config.toml`

### Example Configuration

```toml
# Directories to monitor recursively
watch_paths = [
    "~/Downloads",
    "~/Desktop",
    "~/Projects"
]

# Glob patterns to exclude from monitoring
ignore_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/.venv/**",
    "**/*.tmp",
    "**/*.swp"
]

# Minimum file size to log (bytes, 0 = all files)
min_size_bytes = 0

# Retention period for old entries (days, 0 = no cleanup)
retention_days = 90

# Log level (error, warn, info, debug, trace)
log_level = "info"
```

### Hidden Files and .venv

By default, Ferret monitors all files including those in hidden directories like `.venv`. To exclude hidden directories, add the pattern to `ignore_patterns`:

```toml
ignore_patterns = [
    "/**/.*/**",  # Exclude all hidden directories
]
```

## Commands

### watch
Start the file watcher with interactive TUI or headless mode.

```bash
ferret-tracker watch [OPTIONS]

Options:
  --watch <PATH>    Add directory to watch (can be repeated)
  --headless        Run without TUI (background mode)
  --no-defaults     Ignore paths in config file
```

### list
Display recent file events from the database.

```bash
ferret-tracker list [OPTIONS]

Options:
  --since <DURATION>    Time filter (e.g., "24h", "7d")
  --type <TYPE>         Filter by file type
  --path <PATTERN>      Filter by path substring
  -n, --limit <N>       Maximum entries to show (default: 50)
  --json                Output as JSON
```

### stats
Show statistics about tracked files.

```bash
ferret-tracker stats [OPTIONS]

Options:
  --json    Output as JSON
```

## Database

### Location

- **Linux**: `~/.local/share/ferret/ledger.db`
- **macOS**: `~/Library/Application Support/ferret/ledger.db`
- **Windows**: `%LOCALAPPDATA%\ferret\ledger.db`

### Schema

```sql
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    dir TEXT NOT NULL,
    filename TEXT NOT NULL,
    size_bytes INTEGER,
    created_at TEXT NOT NULL,
    file_type TEXT NOT NULL,
    tags TEXT DEFAULT '',
    notes TEXT DEFAULT ''
);
```

## Development

### Prerequisites

- Rust 1.75+
- Cargo

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_tree_nav -- --nocapture
```

### Code Quality

```bash
# Format check
cargo fmt --check

# Lint warnings
cargo clippy

# Fix formatting issues
cargo fmt
```

### Running with Debug Logging

```bash
RUST_LOG=debug cargo run -- watch
```

## Architecture

```
┌─────────────────────────────────────────┐
│         Main/UI Thread                  │
│  ┌──────────┐  ┌──────────┐             │
│  │ Config   │  │ Store    │ SQLite DB   │
│  │ Loader   │  │ (read)   │             │
│  └──────────┘  └──────────┘             │
│         ↑                                │
│      Channel                            │
│         ↓                                │
│  ┌────────────────────────────────┐     │
│  │     Ratatui TUI Engine         │     │
│  └────────────────────────────────┘     │
└─────────────────────────────────────────┘
        ▲
        │ File Events
        │
┌─────────────────────────────────────────┐
│      Watcher Thread                     │
│  ┌────────────────────────────────┐     │
│  │ notify (inotify/FSEvents)      │     │
│  │ → FileEvent → DB insertion     │     │
│  └────────────────────────────────┘     │
└─────────────────────────────────────────┘
```

## Contributing

Contributions are welcome. Please ensure:
- Code passes `cargo test`
- Code is formatted with `cargo fmt`
- Clippy warnings are addressed

## License

MIT License — see [LICENSE](LICENSE) for details.

## Dependencies

- **ratatui** — Terminal UI framework
- **notify** — Cross-platform file system events
- **rusqlite** — SQLite database bindings
- **clap** — Command-line argument parsing
- **serde** — Serialization framework
- **chrono** — Date/time handling

# 🦡 Ferret

**A curious file tracker** — Ferret is a lightweight, terminal-based tool that monitors directories for new files and presents them in an interactive, searchable TUI.

Like its namesake, Ferret is small, fast, and excellent at finding things. It maintains a local ledger of all files that appear in your watched directories, making it easy to track downloads, artifacts, and any other files that flow into your system.

## ✨ Features

- **Real-time file monitoring** — Watches directories for new files using native OS APIs (inotify on Linux, FSEvents on macOS)
- **Local SQLite ledger** — Persistent, searchable history of all detected files
- **Interactive TUI** — Navigate, filter, and act on files with keyboard shortcuts
- **Smart classification** — Automatically categorizes files by type (executable, archive, document, media, code, etc.)
- **Configurable** — TOML configuration with CLI overrides
- **Lightweight** — Event-driven architecture with minimal CPU usage

## 📦 Installation

### From crates.io

```bash
cargo install ferret-tracker
```

### From Source

```bash
# Clone the repository
git clone https://github.com/mohammad-albarham/ferret.git
cd ferret

# Build in release mode
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .
```

### Requirements

- Rust 1.75 or later
- SQLite (bundled with rusqlite)
- Linux, macOS, or Windows

## 🚀 Quick Start

```bash
# Start watching with TUI (uses default config)
ferret-tracker watch

# Watch specific directories
ferret-tracker watch --watch ~/Downloads --watch ~/Desktop

# Run headless (no TUI, just logging)
ferret-tracker watch --headless

# List recent files
ferret-tracker list --since 24h

# Show statistics
ferret-tracker stats
```

## ⌨️ TUI Keybindings

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate list |
| `PgUp/PgDn` | Scroll by page |
| `Home/End` | Jump to start/end |
| `Enter` | View details |
| `f` | Open filter menu |
| `/` | Search by path |
| `o` | Open file/folder |
| `t` | Edit tags |
| `n` | Edit notes |
| `d` | Delete file (with confirmation) |
| `r` | Refresh list |
| `q` or `Esc` | Quit / Close overlay |
| `?` | Show help |

## ⚙️ Configuration

Ferret uses a TOML configuration file located at:
- Linux/macOS: `~/.config/ferret/config.toml`
- Windows: `%APPDATA%\ferret\config.toml`

### Example Configuration

```toml
# Directories to watch (recursive)
watch_paths = [
    "~/Downloads",
    "~/Desktop",
]

# Patterns to ignore (glob syntax)
ignore_patterns = [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**",
    "**/.*",           # Hidden files
    "**/*.tmp",
    "**/*.swp",
]

# Minimum file size to log (in bytes)
# Set to 0 to log all files
min_size_bytes = 0

# Days to keep events before auto-cleanup
# Set to 0 to disable cleanup
retention_days = 90

# Log level: "error", "warn", "info", "debug", "trace"
log_level = "info"

# Database location (default: ~/.local/share/ferret/ledger.db)
# database_path = "~/.local/share/ferret/ledger.db"
```

## 📊 Commands

### `ferret watch`

Start the file watcher with interactive TUI.

```bash
ferret watch [OPTIONS]

Options:
  -w, --watch <PATH>    Additional paths to watch (can be repeated)
      --headless        Run without TUI (daemon mode)
      --no-defaults     Don't use paths from config file
  -h, --help            Show help
```

### `ferret list`

Display recent file events in tabular format.

```bash
ferret list [OPTIONS]

Options:
      --since <DURATION>     Time window (e.g., "24h", "7d", "30d")
      --size-min <BYTES>     Minimum file size filter
      --size-max <BYTES>     Maximum file size filter
      --type <TYPE>          Filter by type (executable, archive, document, media, code, other)
      --path <PATTERN>       Filter by path substring
  -n, --limit <N>            Maximum entries to show (default: 50)
      --json                 Output as JSON
  -h, --help                 Show help
```

### `ferret stats`

Show statistics about tracked files.

```bash
ferret stats [OPTIONS]

Options:
      --json    Output as JSON
  -h, --help    Show help
```

## 🗃️ Database

Ferret stores its ledger in a SQLite database:
- Linux: `~/.local/share/ferret/ledger.db`
- macOS: `~/Library/Application Support/ferret/ledger.db`
- Windows: `%LOCALAPPDATA%\ferret\ledger.db`

### Schema

```sql
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    dir TEXT NOT NULL,
    filename TEXT NOT NULL,
    size_bytes INTEGER,
    created_at TEXT NOT NULL,      -- ISO 8601 timestamp
    file_type TEXT NOT NULL,
    tags TEXT DEFAULT '',          -- Comma-separated
    notes TEXT DEFAULT ''
);
```

## 🔧 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Main Thread                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Config    │  │   Store     │  │        TUI          │  │
│  │   Loader    │  │  (SQLite)   │  │     (Ratatui)       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           ▲
                           │ Channel
                           │
┌─────────────────────────────────────────────────────────────┐
│                     Watcher Thread                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              notify (inotify/FSEvents)              │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## 🛠️ Development

```bash
# Run in debug mode
cargo run -- watch

# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run -- watch

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

## 🗺️ Roadmap

- [ ] Background daemon mode with systemd/launchd integration
- [ ] File deduplication detection (hash-based)
- [ ] Export ledger to CSV/JSON
- [ ] Desktop notifications for large files
- [ ] Network share monitoring
- [ ] File preview in detail view
- [ ] Batch operations on selected files

## 📄 License

This project is dedicated to the public domain under the [CC0 1.0 Universal](LICENSE) license.

## 🙏 Acknowledgments

Built with these excellent Rust crates:
- [Ratatui](https://ratatui.rs/) — Terminal UI framework
- [notify](https://docs.rs/notify/) — Cross-platform file system notifications
- [rusqlite](https://docs.rs/rusqlite/) — SQLite bindings
- [clap](https://docs.rs/clap/) — Command line argument parsing

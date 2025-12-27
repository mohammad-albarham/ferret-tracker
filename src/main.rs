//! Ferret - A curious file tracker
//!
//! Ferret is a lightweight, terminal-based tool that monitors directories
//! for new files and presents them in an interactive TUI. It maintains
//! a local ledger of all files that appear in watched directories,
//! making it easy to track downloads, artifacts, and file flow.

mod config;
mod models;
mod store;
mod tui;
mod watcher;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::config::{default_config_toml, validate_config, CliOverrides, Config};
use crate::models::{EventFilter, FileType};
use crate::store::Store;
use crate::tui::{app::run_tui, App};
use crate::watcher::FileWatcher;

/// 🦡 Ferret - A curious file tracker
#[derive(Parser)]
#[command(name = "ferret")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to config file (default: ~/.config/ferret/config.toml)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start watching directories with interactive TUI
    Watch {
        /// Additional paths to watch (can be specified multiple times)
        #[arg(short, long)]
        watch: Vec<PathBuf>,

        /// Run without TUI (headless/daemon mode)
        #[arg(long)]
        headless: bool,

        /// Don't use default paths from config
        #[arg(long)]
        no_defaults: bool,
    },

    /// List recent file events
    List {
        /// Time window (e.g., "1h", "24h", "7d", "30d")
        #[arg(long)]
        since: Option<String>,

        /// Minimum file size in bytes
        #[arg(long)]
        size_min: Option<u64>,

        /// Maximum file size in bytes
        #[arg(long)]
        size_max: Option<u64>,

        /// Filter by file type
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,

        /// Filter by path substring
        #[arg(long)]
        path: Option<String>,

        /// Maximum number of entries to show
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show statistics about tracked files
    Stats {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show or create configuration
    Config {
        /// Show current configuration path
        #[arg(long)]
        path: bool,

        /// Initialize default config file
        #[arg(long)]
        init: bool,

        /// Show example configuration
        #[arg(long)]
        example: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine if we'll be running in TUI mode (needed before logging setup)
    let tui_mode = match &cli.command {
        Some(Commands::Watch { headless, .. }) => !headless,
        None => true, // Default command runs TUI
        _ => false,
    };

    // Initialize logging (disabled in TUI mode to prevent screen corruption)
    setup_logging(&cli.log_level, tui_mode)?;

    // Load configuration
    let config = load_config(&cli)?;

    // Execute command
    match cli.command {
        Some(Commands::Watch {
            watch,
            headless,
            no_defaults,
        }) => {
            let overrides = CliOverrides {
                watch_paths: watch,
                no_defaults,
                ..Default::default()
            };
            cmd_watch(config.with_cli_overrides(overrides), headless)
        }
        Some(Commands::List {
            since,
            size_min,
            size_max,
            r#type,
            path,
            limit,
            json,
        }) => cmd_list(config, since, size_min, size_max, r#type, path, limit, json),
        Some(Commands::Stats { json }) => cmd_stats(config, json),
        Some(Commands::Config {
            path,
            init,
            example,
        }) => cmd_config(path, init, example),
        None => {
            // Default to watch command with TUI
            cmd_watch(config, false)
        }
    }
}

/// Setup logging with tracing
fn setup_logging(level: &str, tui_mode: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if tui_mode {
        // In TUI mode, disable logging to avoid interfering with the display
        // Logs would corrupt the TUI rendering
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("off"))
            .with_target(false)
            .without_time()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .without_time()
            .init();
    }

    Ok(())
}

/// Load configuration from file
fn load_config(cli: &Cli) -> Result<Config> {
    let config = if let Some(config_path) = &cli.config {
        Config::load_from_file(config_path)?
    } else {
        Config::load().unwrap_or_else(|e| {
            warn!("Failed to load config: {}. Using defaults.", e);
            Config::default()
        })
    };

    Ok(config)
}

/// Watch command - start monitoring with optional TUI
fn cmd_watch(config: Config, headless: bool) -> Result<()> {
    // Validate configuration
    validate_config(&config)?;

    let watch_paths = config.expanded_watch_paths();
    info!("Starting Ferret with {} watch paths", watch_paths.len());

    // Initialize database
    let db_path = config.database_path();
    let store = Store::new(&db_path).context("Failed to initialize database")?;

    // Run retention cleanup
    if config.retention_days > 0 {
        let cleaned = store.cleanup_old_events(config.retention_days)?;
        if cleaned > 0 {
            info!("Cleaned up {} old events", cleaned);
        }
    }

    // Initialize file watcher
    let (mut watcher, watcher_rx) =
        FileWatcher::new(&config, Some(store.clone())).context("Failed to create file watcher")?;

    // Start watching paths
    watcher
        .watch_paths(&watch_paths)
        .context("Failed to start watching paths")?;

    if headless {
        // Headless mode - just log events
        info!("Running in headless mode. Press Ctrl+C to stop.");

        loop {
            match watcher_rx.recv() {
                Ok(msg) => match msg {
                    watcher::WatcherMessage::NewFile(event) => {
                        store.insert_event(&event)?;
                        info!(
                            "New file: {} ({}, {})",
                            event.path.display(),
                            event.file_type,
                            event.size_display()
                        );
                    }
                    watcher::WatcherMessage::MovedFile(event) => {
                        store.insert_event(&event)?;
                        info!("Moved file: {} ({})", event.path.display(), event.file_type);
                    }
                    watcher::WatcherMessage::Error(err) => {
                        error!("Watcher error: {}", err);
                    }
                    watcher::WatcherMessage::Started => {
                        info!("Watcher started");
                    }
                    watcher::WatcherMessage::Stopped => {
                        info!("Watcher stopped");
                        break;
                    }
                },
                Err(e) => {
                    error!("Channel error: {}", e);
                    break;
                }
            }
        }
    } else {
        // TUI mode
        let mut app = App::new(store)?;
        app.set_watched_dirs(watch_paths.len());

        run_tui(app, Some(watcher_rx))?;
    }

    // Cleanup
    watcher.stop()?;

    Ok(())
}

/// List command - show recent events
fn cmd_list(
    config: Config,
    since: Option<String>,
    size_min: Option<u64>,
    size_max: Option<u64>,
    file_type: Option<String>,
    path_filter: Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let db_path = config.database_path();

    if !db_path.exists() {
        println!("{}", "No database found. Run 'ferret watch' first.".yellow());
        return Ok(());
    }

    let store = Store::new(&db_path)?;

    // Build filter
    let mut filter = EventFilter::new().with_limit(limit);

    if let Some(since_str) = since {
        let duration = parse_duration(&since_str)?;
        filter = filter.with_since(Utc::now() - duration);
    }

    if let Some(min) = size_min {
        filter = filter.with_min_size(min);
    }

    if let Some(max) = size_max {
        filter = filter.with_max_size(max);
    }

    if let Some(type_str) = file_type {
        let ft = type_str
            .parse::<FileType>()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        filter = filter.with_type(ft);
    }

    if let Some(path) = path_filter {
        filter = filter.with_path_contains(&path);
    }

    let events = store.query_events(&filter)?;

    if json {
        let json_output = serde_json::to_string_pretty(&events)?;
        println!("{}", json_output);
    } else {
        if events.is_empty() {
            println!("{}", "No matching events found.".yellow());
            return Ok(());
        }

        // Print table header
        println!(
            "{:19} {:>10} {:6} {}",
            "TIME".bold(),
            "SIZE".bold(),
            "TYPE".bold(),
            "PATH".bold()
        );
        println!("{}", "─".repeat(80));

        for event in events {
            let time = event
                .created_at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S");
            let size = event.size_display();
            let file_type = format_file_type(event.file_type);
            let path = event.path.to_string_lossy();

            println!("{:19} {:>10} {:6} {}", time, size, file_type, path);
        }
    }

    Ok(())
}

/// Stats command - show statistics
fn cmd_stats(config: Config, json: bool) -> Result<()> {
    let db_path = config.database_path();

    if !db_path.exists() {
        println!("{}", "No database found. Run 'ferret watch' first.".yellow());
        return Ok(());
    }

    let store = Store::new(&db_path)?;
    let stats = store.get_stats()?;

    if json {
        let json_output = serde_json::to_string_pretty(&stats)?;
        println!("{}", json_output);
    } else {
        println!("{}", "🦡 Ferret Statistics".bold().cyan());
        println!("{}", "═".repeat(50));

        println!("\n{}", "Overall".bold().yellow());
        println!("  Total files tracked: {}", stats.total_count);
        println!("  Total size: {}", stats.total_size_display());

        println!("\n{}", "Time Periods".bold().yellow());
        println!(
            "  Last 24h: {} files ({} total)",
            stats.count_24h,
            stats.size_24h_display()
        );
        println!(
            "  Last 7d:  {} files ({} total)",
            stats.count_7d,
            stats.size_7d_display()
        );
        println!(
            "  Last 30d: {} files ({} total)",
            stats.count_30d,
            stats.size_30d_display()
        );

        if !stats.by_type.is_empty() {
            println!("\n{}", "By File Type".bold().yellow());
            for (file_type, count, size) in &stats.by_type {
                let size_str = humansize::format_size(*size, humansize::BINARY);
                println!("  {:10} {:5} files ({:>10})", file_type, count, size_str);
            }
        }

        if !stats.top_dirs.is_empty() {
            println!("\n{}", "Top Directories".bold().yellow());
            for (dir, count, size) in stats.top_dirs.iter().take(5) {
                let size_str = humansize::format_size(*size, humansize::BINARY);
                let dir_name = dir
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("?");
                println!("  {:20} {:5} files ({:>10})", dir_name, count, size_str);
            }
        }
    }

    Ok(())
}

/// Config command - show or manage configuration
fn cmd_config(show_path: bool, init: bool, example: bool) -> Result<()> {
    if example {
        println!("{}", default_config_toml());
        return Ok(());
    }

    if init {
        let path = Config::ensure_default_config()?;
        println!("{} {}", "Created config file:".green(), path.display());
        return Ok(());
    }

    if show_path {
        let path = Config::config_file_path();
        println!("{}", path.display());
        if !path.exists() {
            println!("{}", "(file does not exist yet)".yellow());
        }
        return Ok(());
    }

    // Default: show current config
    let config = Config::load()?;
    let config_toml = toml::to_string_pretty(&config)?;
    println!("{}", config_toml);

    Ok(())
}

/// Parse duration string like "1h", "24h", "7d", "30d"
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim().to_lowercase();

    if let Some(hours) = s.strip_suffix('h') {
        let num: i64 = hours.parse().context("Invalid hours value")?;
        return Ok(Duration::hours(num));
    }

    if let Some(days) = s.strip_suffix('d') {
        let num: i64 = days.parse().context("Invalid days value")?;
        return Ok(Duration::days(num));
    }

    // Try parsing as hours if no suffix
    let num: i64 = s.parse().context("Invalid duration format. Use '24h' or '7d'")?;
    Ok(Duration::hours(num))
}

/// Format file type with color
fn format_file_type(ft: FileType) -> String {
    match ft {
        FileType::Executable => ft.as_label().red().to_string(),
        FileType::Archive => ft.as_label().magenta().to_string(),
        FileType::Document => ft.as_label().blue().to_string(),
        FileType::Media => ft.as_label().green().to_string(),
        FileType::Code => ft.as_label().yellow().to_string(),
        FileType::Other => ft.as_label().white().to_string(),
    }
}

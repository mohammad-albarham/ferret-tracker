//! Data models for Ferret
//!
//! This module contains the core data structures used throughout the application,
//! including file events, file type classifications, and filter criteria.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Classification of file types based on extension and heuristics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    /// Executable files (.exe, .app, .sh, ELF binaries, etc.)
    Executable,
    /// Archive files (.zip, .tar, .gz, .rar, etc.)
    Archive,
    /// Document files (.pdf, .doc, .txt, .md, etc.)
    Document,
    /// Media files (.jpg, .png, .mp3, .mp4, etc.)
    Media,
    /// Source code files (.rs, .py, .js, .c, etc.)
    Code,
    /// Unknown or unclassified files
    #[default]
    Other,
}

impl FileType {
    /// Classify a file based on its extension
    pub fn from_extension(ext: &str) -> Self {
        let ext_lower = ext.to_lowercase();
        match ext_lower.as_str() {
            // Executables
            "exe" | "msi" | "app" | "deb" | "rpm" | "sh" | "bash" | "zsh" | "bat"
            | "cmd" | "ps1" | "appimage" | "run" | "bin" | "com" => FileType::Executable,

            // Archives (dmg here, not in executables)
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" | "tbz2" | "txz" | "lz"
            | "lzma" | "lzo" | "z" | "cab" | "iso" | "img" | "dmg" | "pkg" | "jar" | "war"
            | "ear" => FileType::Archive,

            // Documents
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "markdown" | "tex" | "latex"
            | "xls" | "xlsx" | "ods" | "csv" | "ppt" | "pptx" | "odp" | "epub" | "mobi" | "azw"
            | "djvu" | "pages" | "numbers" | "key" | "org" | "rst" | "adoc" | "asciidoc" => {
                FileType::Document
            }

            // Media - Images
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "tif" | "webp" | "svg" | "ico"
            | "heic" | "heif" | "raw" | "cr2" | "nef" | "arw" | "dng" | "psd" | "ai" | "xcf"
            | "avif" => FileType::Media,

            // Media - Audio
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" | "ape"
            | "alac" | "mid" | "midi" => FileType::Media,

            // Media - Video (ts removed - conflicts with TypeScript)
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg"
            | "3gp" | "ogv" | "vob" | "m2ts" | "mts" => FileType::Media,

            // Code - Common languages (removed duplicates: ts, jsx, el)
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "h" | "cpp" | "hpp" | "cc" | "cxx"
            | "java" | "kt" | "kts" | "scala" | "go" | "rb" | "php" | "swift" | "m" | "mm"
            | "cs" | "fs" | "fsx" | "vb" | "pl" | "pm" | "lua" | "r" | "jl" | "ex" | "exs"
            | "erl" | "hrl" | "hs" | "lhs" | "ml" | "mli" | "clj" | "cljs" | "cljc" | "lisp"
            | "el" | "scm" | "rkt" | "nim" | "zig" | "v" | "d" | "ada" | "adb" | "ads" | "pas"
            | "pp" | "f" | "f90" | "f95" | "for" | "cob" | "cbl" | "asm" | "s" => FileType::Code,

            // Code - Web & Config
            "html" | "htm" | "xhtml" | "css" | "scss" | "sass" | "less" | "vue" | "svelte"
            | "json" | "yaml" | "yml" | "toml" | "xml" | "xsl" | "xslt" | "dtd" | "xsd"
            | "graphql" | "gql" | "sql" | "prisma" => FileType::Code,

            // Code - Shell & Scripts
            "fish" | "nu" | "awk" | "sed" | "vim" => FileType::Code,

            // Code - Build & Config files
            "makefile" | "cmake" | "dockerfile" | "containerfile" | "gradle" | "sbt" | "pom"
            | "cabal" | "stack" | "nix" | "dhall" => FileType::Code,

            _ => FileType::Other,
        }
    }

    /// Classify a file based on its path (extension + special filename handling)
    pub fn from_path(path: &Path) -> Self {
        // Check for special filenames first
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            let filename_lower = filename.to_lowercase();

            // Special code-related files
            match filename_lower.as_str() {
                "makefile" | "gnumakefile" | "cmakelists.txt" | "dockerfile" | "containerfile"
                | "vagrantfile" | "rakefile" | "gemfile" | "procfile" | "brewfile"
                | "justfile" => return FileType::Code,
                _ => {}
            }

            // Check if it's a hidden dotfile config
            if filename.starts_with('.') {
                let without_dot = &filename[1..];
                match without_dot.to_lowercase().as_str() {
                    "gitignore" | "gitattributes" | "gitmodules" | "editorconfig" | "prettierrc"
                    | "eslintrc" | "babelrc" | "npmrc" | "yarnrc" | "dockerignore" | "env"
                    | "envrc" => return FileType::Code,
                    _ => {}
                }
            }
        }

        // Fall back to extension-based classification
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(FileType::from_extension)
            .unwrap_or(FileType::Other)
    }

    /// Check if a file might be executable based on Unix permissions
    #[cfg(unix)]
    pub fn check_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            let mode = metadata.permissions().mode();
            // Check if any execute bit is set
            mode & 0o111 != 0
        } else {
            false
        }
    }

    #[cfg(not(unix))]
    pub fn check_executable(_path: &Path) -> bool {
        false
    }

    /// Returns a short display label for the file type
    pub fn as_label(&self) -> &'static str {
        match self {
            FileType::Executable => "exec",
            FileType::Archive => "arch",
            FileType::Document => "doc",
            FileType::Media => "media",
            FileType::Code => "code",
            FileType::Other => "other",
        }
    }

    /// Returns a descriptive name for the file type
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Executable => "executable",
            FileType::Archive => "archive",
            FileType::Document => "document",
            FileType::Media => "media",
            FileType::Code => "code",
            FileType::Other => "other",
        }
    }

    /// All file type variants for iteration
    pub fn all() -> &'static [FileType] {
        &[
            FileType::Executable,
            FileType::Archive,
            FileType::Document,
            FileType::Media,
            FileType::Code,
            FileType::Other,
        ]
    }
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for FileType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "executable" | "exec" => Ok(FileType::Executable),
            "archive" | "arch" => Ok(FileType::Archive),
            "document" | "doc" => Ok(FileType::Document),
            "media" => Ok(FileType::Media),
            "code" => Ok(FileType::Code),
            "other" => Ok(FileType::Other),
            _ => Err(format!("Unknown file type: {}", s)),
        }
    }
}

/// Represents a file event recorded in the ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvent {
    /// Unique identifier (database row ID)
    pub id: Option<i64>,
    /// Full absolute path to the file
    pub path: PathBuf,
    /// Parent directory
    pub dir: PathBuf,
    /// Filename (without directory)
    pub filename: String,
    /// File size in bytes (if available)
    pub size_bytes: Option<u64>,
    /// When the file was first seen (UTC)
    pub created_at: DateTime<Utc>,
    /// Classified file type
    pub file_type: FileType,
    /// User-defined tags (comma-separated)
    pub tags: String,
    /// User-defined notes
    pub notes: String,
}

impl FileEvent {
    /// Create a new FileEvent from a path
    pub fn from_path(path: PathBuf) -> Self {
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_string();

        // Get file size if accessible
        let size_bytes = std::fs::metadata(&path).ok().map(|m| m.len());

        // Classify file type
        let mut file_type = FileType::from_path(&path);

        // If type is Other but file is executable, classify as Executable
        if file_type == FileType::Other && FileType::check_executable(&path) {
            file_type = FileType::Executable;
        }

        Self {
            id: None,
            path,
            dir,
            filename,
            size_bytes,
            created_at: Utc::now(),
            file_type,
            tags: String::new(),
            notes: String::new(),
        }
    }

    /// Format size for display
    pub fn size_display(&self) -> String {
        match self.size_bytes {
            Some(size) => humansize::format_size(size, humansize::BINARY),
            None => "—".to_string(),
        }
    }

    /// Get tags as a vector
    pub fn tags_vec(&self) -> Vec<&str> {
        if self.tags.is_empty() {
            Vec::new()
        } else {
            self.tags.split(',').map(|s| s.trim()).collect()
        }
    }

    /// Set tags from a vector
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags.join(", ");
    }
}

/// Filter criteria for querying events
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Filter by file type
    pub file_type: Option<FileType>,
    /// Filter by minimum size in bytes
    pub min_size: Option<u64>,
    /// Filter by maximum size in bytes
    pub max_size: Option<u64>,
    /// Filter by path substring
    pub path_contains: Option<String>,
    /// Filter events after this time
    pub since: Option<DateTime<Utc>>,
    /// Filter events before this time
    pub until: Option<DateTime<Utc>>,
    /// Filter by specific directory
    pub dir: Option<PathBuf>,
    /// Maximum number of results (for pagination)
    pub limit: usize,
    /// Offset for pagination
    pub offset: usize,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            file_type: None,
            min_size: None,
            max_size: None,
            path_contains: None,
            since: None,
            until: None,
            dir: None,
            limit: 100, // Default page size
            offset: 0,
        }
    }
}

impl EventFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by file type
    pub fn with_type(mut self, file_type: FileType) -> Self {
        self.file_type = Some(file_type);
        self
    }

    /// Filter by minimum size
    pub fn with_min_size(mut self, size: u64) -> Self {
        self.min_size = Some(size);
        self
    }

    /// Filter by maximum size
    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_size = Some(size);
        self
    }

    /// Filter by path substring
    pub fn with_path_contains(mut self, pattern: &str) -> Self {
        self.path_contains = Some(pattern.to_string());
        self
    }

    /// Filter events since a specific time
    pub fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Filter events in the last N hours
    pub fn with_last_hours(mut self, hours: i64) -> Self {
        self.since = Some(Utc::now() - chrono::Duration::hours(hours));
        self
    }

    /// Filter events in the last N days
    pub fn with_last_days(mut self, days: i64) -> Self {
        self.since = Some(Utc::now() - chrono::Duration::days(days));
        self
    }

    /// Filter by directory
    pub fn with_dir(mut self, dir: PathBuf) -> Self {
        self.dir = Some(dir);
        self
    }

    /// Limit results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set pagination offset
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Set pagination (limit and offset)
    pub fn with_pagination(mut self, limit: usize, offset: usize) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }

    /// Check if filter is empty (no criteria set)
    pub fn is_empty(&self) -> bool {
        self.file_type.is_none()
            && self.min_size.is_none()
            && self.max_size.is_none()
            && self.path_contains.is_none()
            && self.since.is_none()
            && self.until.is_none()
            && self.dir.is_none()
    }

    /// Generate a human-readable summary of active filters
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ft) = &self.file_type {
            parts.push(format!("type:{}", ft.as_label()));
        }
        if let Some(min) = self.min_size {
            parts.push(format!("≥{}", humansize::format_size(min, humansize::BINARY)));
        }
        if let Some(max) = self.max_size {
            parts.push(format!("≤{}", humansize::format_size(max, humansize::BINARY)));
        }
        if let Some(path) = &self.path_contains {
            parts.push(format!("path:*{}*", path));
        }
        if let Some(since) = &self.since {
            let duration = Utc::now() - *since;
            if duration.num_hours() < 24 {
                parts.push(format!("last {}h", duration.num_hours()));
            } else {
                parts.push(format!("last {}d", duration.num_days()));
            }
        }
        if let Some(dir) = &self.dir {
            parts.push(format!(
                "dir:{}",
                dir.file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("?")
            ));
        }

        if parts.is_empty() {
            "No filters".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

/// Statistics about tracked files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventStats {
    /// Total number of events
    pub total_count: u64,
    /// Total size of all tracked files
    pub total_size: u64,
    /// Events in last 24 hours
    pub count_24h: u64,
    /// Size in last 24 hours
    pub size_24h: u64,
    /// Events in last 7 days
    pub count_7d: u64,
    /// Size in last 7 days
    pub size_7d: u64,
    /// Events in last 30 days
    pub count_30d: u64,
    /// Size in last 30 days
    pub size_30d: u64,
    /// Breakdown by file type
    pub by_type: Vec<(FileType, u64, u64)>, // (type, count, size)
    /// Top directories by volume
    pub top_dirs: Vec<(PathBuf, u64, u64)>, // (dir, count, size)
}

impl EventStats {
    /// Format total size for display
    pub fn total_size_display(&self) -> String {
        humansize::format_size(self.total_size, humansize::BINARY)
    }

    /// Format 24h size for display
    pub fn size_24h_display(&self) -> String {
        humansize::format_size(self.size_24h, humansize::BINARY)
    }

    /// Format 7d size for display
    pub fn size_7d_display(&self) -> String {
        humansize::format_size(self.size_7d, humansize::BINARY)
    }

    /// Format 30d size for display
    pub fn size_30d_display(&self) -> String {
        humansize::format_size(self.size_30d, humansize::BINARY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_from_extension() {
        assert_eq!(FileType::from_extension("exe"), FileType::Executable);
        assert_eq!(FileType::from_extension("EXE"), FileType::Executable);
        assert_eq!(FileType::from_extension("zip"), FileType::Archive);
        assert_eq!(FileType::from_extension("pdf"), FileType::Document);
        assert_eq!(FileType::from_extension("mp3"), FileType::Media);
        assert_eq!(FileType::from_extension("rs"), FileType::Code);
        assert_eq!(FileType::from_extension("xyz"), FileType::Other);
    }

    #[test]
    fn test_file_type_from_path() {
        assert_eq!(
            FileType::from_path(Path::new("/home/user/file.rs")),
            FileType::Code
        );
        assert_eq!(
            FileType::from_path(Path::new("Makefile")),
            FileType::Code
        );
        assert_eq!(
            FileType::from_path(Path::new(".gitignore")),
            FileType::Code
        );
        assert_eq!(
            FileType::from_path(Path::new("photo.jpg")),
            FileType::Media
        );
    }

    #[test]
    fn test_file_type_parse() {
        assert_eq!("executable".parse::<FileType>().unwrap(), FileType::Executable);
        assert_eq!("arch".parse::<FileType>().unwrap(), FileType::Archive);
        assert_eq!("MEDIA".parse::<FileType>().unwrap(), FileType::Media);
    }

    #[test]
    fn test_event_filter_summary() {
        let filter = EventFilter::new()
            .with_type(FileType::Archive)
            .with_min_size(1024 * 1024);
        
        let summary = filter.summary();
        assert!(summary.contains("type:arch"));
        assert!(summary.contains("≥1 MiB"));
    }

    #[test]
    fn test_file_event_tags() {
        let mut event = FileEvent::from_path(PathBuf::from("/tmp/test.txt"));
        assert!(event.tags_vec().is_empty());
        
        event.tags = "important, backup".to_string();
        let tags = event.tags_vec();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], "important");
        assert_eq!(tags[1], "backup");
    }
}

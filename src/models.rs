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

// ============================================================================
// View Mode and Tree View Types
// ============================================================================

/// View mode for the file list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Flat chronological list (default)
    #[default]
    Flat,
    /// Group files by folder (single level)
    GroupByFolder,
    /// Full nested tree hierarchy
    TreeView,
}

impl ViewMode {
    /// Cycle to next view mode
    pub fn next(&self) -> Self {
        match self {
            ViewMode::Flat => ViewMode::GroupByFolder,
            ViewMode::GroupByFolder => ViewMode::TreeView,
            ViewMode::TreeView => ViewMode::Flat,
        }
    }
    
    /// Get display name for the view mode
    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::Flat => "Flat",
            ViewMode::GroupByFolder => "Grouped",
            ViewMode::TreeView => "Tree",
        }
    }
}

/// Type of node in the tree view
#[derive(Debug, Clone)]
pub enum TreeNodeType {
    /// A directory that may contain children
    Directory,
    /// A file with associated FileEvent data
    File(Box<FileEvent>),
}

/// A node in the file tree
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Display name (directory name or filename)
    pub name: String,
    /// Full path identifier
    pub path: PathBuf,
    /// Node type (directory or file)
    pub node_type: TreeNodeType,
    /// Children nodes (directories first, then files)
    pub children: Vec<TreeNode>,
    /// Total file count in this subtree
    pub file_count: usize,
    /// Total size of files in this subtree
    pub total_size: u64,
}

impl TreeNode {
    /// Build tree from flat list of FileEvents
    pub fn from_events(events: &[FileEvent]) -> Vec<TreeNode> {
        use std::collections::BTreeMap;
        
        if events.is_empty() {
            return Vec::new();
        }
        
        // Find the common root path prefix
        let common_root = Self::find_common_root(events);
        
        // Group events by their directory paths
        let mut dir_files: BTreeMap<PathBuf, Vec<&FileEvent>> = BTreeMap::new();
        for event in events {
            dir_files.entry(event.dir.clone())
                .or_default()
                .push(event);
        }
        
        // Build hierarchical structure starting from common root
        Self::build_subtree(&dir_files, &common_root)
    }
    
    /// Find the common root path for all events
    fn find_common_root(events: &[FileEvent]) -> PathBuf {
        if events.is_empty() {
            return PathBuf::new();
        }
        
        let first_dir = &events[0].dir;
        let mut common: Vec<_> = first_dir.components().collect();
        
        for event in events.iter().skip(1) {
            let components: Vec<_> = event.dir.components().collect();
            let mut new_common = Vec::new();
            
            for (a, b) in common.iter().zip(components.iter()) {
                if a == b {
                    new_common.push(*a);
                } else {
                    break;
                }
            }
            common = new_common;
        }
        
        common.iter().collect()
    }
    
    /// Recursively build subtree from grouped files
    fn build_subtree(
        dir_files: &std::collections::BTreeMap<PathBuf, Vec<&FileEvent>>,
        current_path: &PathBuf,
    ) -> Vec<TreeNode> {
        let mut nodes = Vec::new();
        let mut seen_dirs = std::collections::HashSet::new();
        
        // Find all directories that are immediate children of current_path
        for dir_path in dir_files.keys() {
            if dir_path == current_path {
                continue;
            }
            
            // Check if this directory is under current_path
            if let Ok(rel) = dir_path.strip_prefix(current_path) {
                // Get first component (immediate child dir)
                if let Some(first_component) = rel.components().next() {
                    let child_path = current_path.join(first_component);
                    
                    if seen_dirs.insert(child_path.clone()) {
                        // Recursively build children
                        let children = Self::build_subtree(dir_files, &child_path);
                        
                        // Get files directly in this directory
                        let mut file_nodes: Vec<TreeNode> = dir_files
                            .get(&child_path)
                            .map(|files| {
                                files.iter().map(|e| TreeNode {
                                    name: e.filename.clone(),
                                    path: e.path.clone(),
                                    node_type: TreeNodeType::File(Box::new((*e).clone())),
                                    children: vec![],
                                    file_count: 1,
                                    total_size: e.size_bytes.unwrap_or(0),
                                }).collect()
                            })
                            .unwrap_or_default();
                        
                        // Calculate totals
                        let child_file_count: usize = children.iter().map(|c| c.file_count).sum();
                        let child_total_size: u64 = children.iter().map(|c| c.total_size).sum();
                        let direct_file_count = file_nodes.len();
                        let direct_total_size: u64 = file_nodes.iter().map(|f| f.total_size).sum();
                        
                        // Combine children: directories first, then files
                        let mut all_children = children;
                        all_children.append(&mut file_nodes);
                        
                        // Sort: directories first, then alphabetically
                        all_children.sort_by(|a, b| {
                            match (&a.node_type, &b.node_type) {
                                (TreeNodeType::Directory, TreeNodeType::File(_)) => std::cmp::Ordering::Less,
                                (TreeNodeType::File(_), TreeNodeType::Directory) => std::cmp::Ordering::Greater,
                                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                            }
                        });
                        
                        let dir_name = first_component.as_os_str()
                            .to_string_lossy()
                            .to_string();
                        
                        nodes.push(TreeNode {
                            name: dir_name,
                            path: child_path,
                            node_type: TreeNodeType::Directory,
                            children: all_children,
                            file_count: child_file_count + direct_file_count,
                            total_size: child_total_size + direct_total_size,
                        });
                    }
                }
            }
        }
        
        // Also add files directly in current_path
        if let Some(files) = dir_files.get(current_path) {
            for event in files {
                nodes.push(TreeNode {
                    name: event.filename.clone(),
                    path: event.path.clone(),
                    node_type: TreeNodeType::File(Box::new((*event).clone())),
                    children: vec![],
                    file_count: 1,
                    total_size: event.size_bytes.unwrap_or(0),
                });
            }
        }
        
        // Sort: directories first, then alphabetically
        nodes.sort_by(|a, b| {
            match (&a.node_type, &b.node_type) {
                (TreeNodeType::Directory, TreeNodeType::File(_)) => std::cmp::Ordering::Less,
                (TreeNodeType::File(_), TreeNodeType::Directory) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        
        nodes
    }
    
    /// Check if this node is a directory
    pub fn is_dir(&self) -> bool {
        matches!(self.node_type, TreeNodeType::Directory)
    }
    
    /// Get the file event if this is a file node
    pub fn file_event(&self) -> Option<&FileEvent> {
        match &self.node_type {
            TreeNodeType::File(e) => Some(e),
            TreeNodeType::Directory => None,
        }
    }
}

/// A flattened node for rendering (includes depth and tree drawing info)
#[derive(Debug, Clone)]
pub struct FlattenedNode {
    /// Reference path to the node
    pub path: PathBuf,
    /// Display name
    pub name: String,
    /// Depth in the tree (0 = root level)
    pub depth: usize,
    /// Whether this is the last sibling at its level
    pub is_last_sibling: bool,
    /// Whether this directory is expanded (only relevant for directories)
    pub is_expanded: bool,
    /// Whether this is a directory
    pub is_dir: bool,
    /// File type (if file)
    pub file_type: Option<FileType>,
    /// File size (if file)
    pub size_bytes: Option<u64>,
    /// File count (for directories)
    pub file_count: usize,
    /// Ancestors' "is_last" status for drawing vertical lines
    pub ancestor_is_last: Vec<bool>,
}

/// State for tree view navigation and expansion
#[derive(Debug, Clone, Default)]
pub struct TreeViewState {
    /// Set of expanded directory paths
    pub expanded: std::collections::HashSet<PathBuf>,
    /// Currently selected index in flattened list
    pub selected_index: usize,
    /// Scroll offset for visible area
    pub scroll_offset: usize,
    /// Cached flattened nodes for current expansion state
    pub flattened: Vec<FlattenedNode>,
}

impl TreeViewState {
    /// Create new tree view state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Toggle expand/collapse for a directory
    pub fn toggle_expanded(&mut self, path: &PathBuf) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.clone());
        }
    }
    
    /// Expand a directory
    pub fn expand(&mut self, path: &PathBuf) {
        self.expanded.insert(path.clone());
    }
    
    /// Collapse a directory
    pub fn collapse(&mut self, path: &PathBuf) {
        self.expanded.remove(path);
    }
    
    /// Expand all directories
    pub fn expand_all(&mut self, nodes: &[TreeNode]) {
        self.expand_recursive(nodes);
    }
    
    fn expand_recursive(&mut self, nodes: &[TreeNode]) {
        for node in nodes {
            if node.is_dir() {
                self.expanded.insert(node.path.clone());
                self.expand_recursive(&node.children);
            }
        }
    }
    
    /// Collapse all directories
    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }
    
    /// Rebuild flattened list from tree nodes
    pub fn rebuild_flattened(&mut self, nodes: &[TreeNode]) {
        self.flattened.clear();
        self.flatten_recursive(nodes, 0, &mut vec![]);
    }
    
    fn flatten_recursive(
        &mut self,
        nodes: &[TreeNode],
        depth: usize,
        ancestor_is_last: &mut Vec<bool>,
    ) {
        let count = nodes.len();
        for (idx, node) in nodes.iter().enumerate() {
            let is_last = idx == count - 1;
            let is_expanded = self.expanded.contains(&node.path);
            
            self.flattened.push(FlattenedNode {
                path: node.path.clone(),
                name: node.name.clone(),
                depth,
                is_last_sibling: is_last,
                is_expanded,
                is_dir: node.is_dir(),
                file_type: node.file_event().map(|e| e.file_type),
                size_bytes: node.file_event().and_then(|e| e.size_bytes),
                file_count: node.file_count,
                ancestor_is_last: ancestor_is_last.clone(),
            });
            
            // Recurse into expanded directories
            if node.is_dir() && is_expanded {
                ancestor_is_last.push(is_last);
                self.flatten_recursive(&node.children, depth + 1, ancestor_is_last);
                ancestor_is_last.pop();
            }
        }
    }
    
    /// Get index of selected item in flattened list
    pub fn get_selected_index(&self) -> usize {
        self.selected_index.min(self.flattened.len().saturating_sub(1))
    }
    
    /// Get the selected node
    pub fn selected_node(&self) -> Option<&FlattenedNode> {
        self.flattened.get(self.selected_index)
    }
    
    /// Get the selected path
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.selected_node().map(|n| &n.path)
    }
    
    /// Move selection up
    pub fn move_up(&mut self) {
        if self.flattened.is_empty() {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
    
    /// Move selection down
    pub fn move_down(&mut self) {
        if self.flattened.is_empty() {
            return;
        }
        if self.selected_index + 1 < self.flattened.len() {
            self.selected_index += 1;
        }
    }
    
    /// Collapse current directory or move to parent
    pub fn collapse_or_parent(&mut self, nodes: &[TreeNode]) {
        if self.flattened.is_empty() {
            return;
        }
        
        let idx = self.get_selected_index();
        let node = &self.flattened[idx];
        let node_path = node.path.clone();
        
        // If it's an expanded directory, collapse it
        if node.is_dir && self.expanded.contains(&node_path) {
            self.collapse(&node_path);
            self.rebuild_flattened(nodes);
            // Selection index stays the same (now on collapsed folder)
            return;
        }
        
        // Otherwise, go to parent directory
        if let Some(parent) = node_path.parent() {
            let parent_path = parent.to_path_buf();
            if let Some(parent_idx) = self.flattened.iter().position(|n| n.path == parent_path) {
                self.selected_index = parent_idx;
            }
        }
    }
    
    /// Expand current directory
    pub fn expand_selected(&mut self, nodes: &[TreeNode]) {
        if self.flattened.is_empty() {
            return;
        }
        
        let idx = self.get_selected_index();
        let node = &self.flattened[idx];
        let node_path = node.path.clone();
        
        if node.is_dir && !self.expanded.contains(&node_path) {
            self.expand(&node_path);
            self.rebuild_flattened(nodes);
            // Selection index stays the same (now on expanded folder)
        }
    }
    
    /// Toggle expand/collapse of selected directory
    pub fn toggle_selected(&mut self, nodes: &[TreeNode]) {
        if self.flattened.is_empty() {
            return;
        }
        
        let idx = self.get_selected_index();
        let node = &self.flattened[idx];
        let node_path = node.path.clone();
        
        if node.is_dir {
            self.toggle_expanded(&node_path);
            self.rebuild_flattened(nodes);
        }
    }
    
    /// Get the selected node's FileEvent (if it's a file)
    pub fn selected_file_event<'a>(&self, nodes: &'a [TreeNode]) -> Option<&'a FileEvent> {
        let selected = self.selected_path()?;
        Self::find_file_event(nodes, selected)
    }
    
    fn find_file_event<'a>(nodes: &'a [TreeNode], path: &PathBuf) -> Option<&'a FileEvent> {
        for node in nodes {
            if &node.path == path {
                return node.file_event();
            }
            if let Some(event) = Self::find_file_event(&node.children, path) {
                return Some(event);
            }
        }
        None
    }
    
    /// Ensure selected index is visible given scroll offset and visible rows
    pub fn ensure_visible(&mut self, visible_rows: usize) {
        let idx = self.get_selected_index();
        if idx < self.scroll_offset {
            self.scroll_offset = idx;
        } else if idx >= self.scroll_offset + visible_rows {
            self.scroll_offset = idx - visible_rows + 1;
        }
    }
}

/// A group of files in a folder (for GroupByFolder view mode)
#[derive(Debug, Clone)]
pub struct FolderGroup {
    /// The folder path
    pub path: PathBuf,
    /// Display name for the folder
    pub name: String,
    /// Files in this folder
    pub files: Vec<FileEvent>,
    /// Whether the folder is expanded in the UI
    pub expanded: bool,
    /// Total size of all files in this folder
    pub total_size: u64,
}

impl FolderGroup {
    /// Build folder groups from flat list of events
    pub fn from_events(events: &[FileEvent]) -> Vec<FolderGroup> {
        use std::collections::BTreeMap;
        
        let mut groups: BTreeMap<PathBuf, Vec<FileEvent>> = BTreeMap::new();
        
        for event in events {
            groups.entry(event.dir.clone())
                .or_default()
                .push(event.clone());
        }
        
        groups.into_iter()
            .map(|(path, files)| {
                let total_size = files.iter().filter_map(|f| f.size_bytes).sum();
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                
                FolderGroup {
                    path,
                    name,
                    files,
                    expanded: true,
                    total_size,
                }
            })
            .collect()
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

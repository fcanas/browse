use crate::config::Settings;
use chrono::{DateTime, Local};
use std::fs::{self, DirEntry};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Maximum size for file content preview (4KB)
const MAX_PREVIEW_SIZE: u64 = 4096;

/// File details for preview display
#[derive(Debug, Clone)]
pub struct FileDetails {
    pub path: PathBuf,
    pub size: u64,
    pub created: Option<DateTime<Local>>,
    pub modified: Option<DateTime<Local>>,
    pub symlink_target: Option<PathBuf>,
    pub content_preview: String,
    pub mime_type: Option<String>,
}

impl FileDetails {
    /// Create file details from a path with safe error handling
    pub fn from_path(path: &Path, config: &Settings) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        
        let created = metadata.created().ok().map(DateTime::from);
        let modified = metadata.modified().ok().map(DateTime::from);
        
        let symlink_target = if metadata.file_type().is_symlink() {
            fs::read_link(path).ok()
        } else {
            None
        };
        
        let mime_type = if metadata.is_file() {
            get_mime_type(path)
        } else {
            None
        };
        
        let content_preview = if metadata.is_file() {
            read_file_preview(path, &mime_type, config)
                .unwrap_or_else(|_| "[Could not read file]".to_string())
        } else {
            "[Not a regular file]".to_string()
        };
        
        Ok(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            created,
            modified,
            symlink_target,
            content_preview,
            mime_type,
        })
    }
}

/// Safely read directory entries with proper filtering and sorting
pub fn read_directory(path: &Path, config: &Settings) -> io::Result<Vec<DirEntry>> {
    let mut entries: Vec<_> = fs::read_dir(path)?
        .filter_map(|entry| match entry {
            Ok(entry) => {
                // Filter hidden files if not showing them
                if !config.show_hidden_files {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with('.') {
                            return None;
                        }
                    }
                }
                Some(entry)
            }
            Err(e) => {
                eprintln!("Warning: Failed to read directory entry: {}", e);
                None
            }
        })
        .collect();
    
    // Sort entries: directories first, then files, both alphabetically
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    
    Ok(entries)
}

/// Get MIME type with fallback to extension-based detection
pub fn get_mime_type(path: &Path) -> Option<String> {
    // First try infer crate for magic number detection
    if let Ok(Some(kind)) = infer::get_from_path(path) {
        return Some(kind.mime_type().to_string());
    }
    
    // Fallback to extension-based detection
    get_mime_type_from_extension(path)
}

/// Get MIME type based on file extension
fn get_mime_type_from_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    
    match extension.as_str() {
        // Text files
        "txt" | "log" => Some("text/plain".to_string()),
        "md" | "markdown" => Some("text/markdown".to_string()),
        "html" | "htm" => Some("text/html".to_string()),
        "xml" => Some("application/xml".to_string()),
        "css" => Some("text/css".to_string()),
        "csv" => Some("text/csv".to_string()),
        
        // Programming languages
        "rs" => Some("text/x-rust".to_string()),
        "js" | "mjs" => Some("application/javascript".to_string()),
        "ts" | "mts" => Some("application/typescript".to_string()),
        "py" | "pyw" => Some("text/x-python".to_string()),
        "java" => Some("text/x-java".to_string()),
        "c" | "cc" | "cpp" | "h" | "hpp" => Some("text/x-c".to_string()),
        "go" => Some("text/x-go".to_string()),
        "rb" => Some("text/x-ruby".to_string()),
        "php" => Some("text/x-php".to_string()),
        "swift" => Some("text/x-swift".to_string()),
        "kt" | "kts" => Some("text/x-kotlin".to_string()),
        "cs" => Some("text/x-csharp".to_string()),
        "pl" => Some("text/x-perl".to_string()),
        "lua" => Some("text/x-lua".to_string()),
        "sql" => Some("text/x-sql".to_string()),
        
        // Configuration files
        "toml" => Some("application/toml".to_string()),
        "json" => Some("application/json".to_string()),
        "yaml" | "yml" => Some("application/x-yaml".to_string()),
        "sh" | "bash" => Some("application/x-sh".to_string()),
        
        // Images
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        "gif" => Some("image/gif".to_string()),
        "svg" => Some("image/svg+xml".to_string()),
        
        // Archives
        "zip" => Some("application/zip".to_string()),
        "gz" => Some("application/gzip".to_string()),
        "tar" => Some("application/x-tar".to_string()),
        
        _ => None,
    }
}

/// Get the appropriate icon for a file or directory
pub fn get_icon(entry: &DirEntry, config: &Settings) -> String {
    if !config.show_icons {
        return String::new();
    }
    
    let path = entry.path();
    
    // Directory icons
    if path.is_dir() {
        return "📁".to_string();
    }
    
    // Symlink icon
    if entry.file_type().map_or(false, |ft| ft.is_symlink()) {
        if let Some(rule) = config.get_rule("symlink") {
            return rule.icon.clone();
        }
        return "🔗".to_string();
    }
    
    // Executable files
    if let Ok(metadata) = entry.metadata() {
        if metadata.permissions().mode() & 0o111 != 0 {
            return "🚀".to_string();
        }
    }
    
    // MIME type-based icons
    if let Some(mime_type) = get_mime_type(&path) {
        if let Some(rule) = config.get_rule(&mime_type) {
            return rule.icon.clone();
        }
    }
    
    // Default file icon
    "📄".to_string()
}

/// Read file content for preview with size limits and encoding safety
fn read_file_preview(path: &Path, mime_type: &Option<String>, config: &Settings) -> io::Result<String> {
    // Check if preview is enabled for this file type
    let can_preview = mime_type
        .as_ref()
        .and_then(|mime_str| config.get_rule(mime_str))
        .map_or(false, |rule| rule.preview);
    
    if !can_preview {
        return Ok(String::new());
    }
    
    // Check file size before reading
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_PREVIEW_SIZE {
        return Ok(format!("[File too large: {} bytes]", metadata.len()));
    }
    
    // Read file content safely
    let file = fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.take(MAX_PREVIEW_SIZE).read_to_end(&mut buffer)?;
    
    // Convert to string, handling invalid UTF-8
    match String::from_utf8(buffer) {
        Ok(content) => Ok(content),
        Err(_) => Ok("[Binary file - cannot preview]".to_string()),
    }
}

/// Check if a path is safe to access (basic security check)
pub fn is_safe_path(path: &Path) -> bool {
    // Reject paths with suspicious components
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            // Reject hidden files that might be sensitive
            if name_str.starts_with('.') && (
                name_str.contains("ssh") || 
                name_str.contains("key") || 
                name_str.contains("secret")
            ) {
                return false;
            }
        }
    }
    
    // Reject very deep paths (potential zip bomb or similar)
    if path.components().count() > 50 {
        return false;
    }
    
    true
} 
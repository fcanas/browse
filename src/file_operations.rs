use crate::config::Settings;
use crate::error::ErrorLog;
use chrono::{DateTime, Local};
use std::fs::{self, DirEntry};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Maximum size for file content preview (4KB)
const MAX_PREVIEW_SIZE: u64 = 4096;

/// Maximum number of directory entries to display (performance limit)
const MAX_DIRECTORY_ENTRIES: usize = 1000;

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

/// Safely read directory entries with error logging
pub fn read_directory_with_error_log(path: &Path, config: &Settings, mut error_log: Option<&mut ErrorLog>) -> io::Result<Vec<DirEntry>> {
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
                let error_msg = format!("Failed to read directory entry: {}", e);
                if let Some(ref mut log) = error_log {
                    log.warning(error_msg, Some("Directory Reading".to_string()));
                } else {
                    eprintln!("Warning: {}", error_msg);
                }
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

    // Limit entries for performance in very large directories
    if entries.len() > MAX_DIRECTORY_ENTRIES {
        entries.truncate(MAX_DIRECTORY_ENTRIES);
        let warning_msg = format!("Directory has more than {} entries, showing first {}",
                                 MAX_DIRECTORY_ENTRIES, MAX_DIRECTORY_ENTRIES);
        if let Some(ref mut log) = error_log {
            log.warning(warning_msg, Some(format!("Directory: {}", path.display())));
        } else {
            eprintln!("Warning: {}", warning_msg);
        }
    }

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
    use std::collections::HashMap;
    use std::sync::OnceLock;

    // Lazy-initialized extension mapping for better performance
    static EXTENSION_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

    let map = EXTENSION_MAP.get_or_init(|| {
        let mut map = HashMap::new();

        // Text files
        map.extend([
            ("txt", "text/plain"), ("log", "text/plain"),
            ("md", "text/markdown"), ("markdown", "text/markdown"),
            ("html", "text/html"), ("htm", "text/html"),
            ("xml", "application/xml"), ("css", "text/css"), ("csv", "text/csv"),
        ]);

        // Programming languages
        map.extend([
            ("rs", "text/x-rust"), ("js", "application/javascript"), ("mjs", "application/javascript"),
            ("ts", "application/typescript"), ("mts", "application/typescript"),
            ("py", "text/x-python"), ("pyw", "text/x-python"), ("java", "text/x-java"),
            ("c", "text/x-c"), ("cc", "text/x-c"), ("cpp", "text/x-c"), ("h", "text/x-c"), ("hpp", "text/x-c"),
            ("go", "text/x-go"), ("rb", "text/x-ruby"), ("php", "text/x-php"), ("swift", "text/x-swift"),
            ("kt", "text/x-kotlin"), ("kts", "text/x-kotlin"), ("cs", "text/x-csharp"),
            ("pl", "text/x-perl"), ("lua", "text/x-lua"), ("sql", "text/x-sql"),
        ]);

        // Configuration files
        map.extend([
            ("toml", "application/toml"), ("json", "application/json"),
            ("yaml", "application/x-yaml"), ("yml", "application/x-yaml"),
            ("sh", "application/x-sh"), ("bash", "application/x-sh"),
        ]);

        // Images
        map.extend([
            ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("png", "image/png"),
            ("gif", "image/gif"), ("svg", "image/svg+xml"),
        ]);

        // Archives
        map.extend([
            ("zip", "application/zip"), ("gz", "application/gzip"), ("tar", "application/x-tar"),
        ]);

        map
    });

    let extension = path.extension()?.to_str()?.to_lowercase();
    map.get(extension.as_str()).map(|&mime| mime.to_string())
}

/// Get the appropriate icon for a file or directory with error logging
pub fn get_icon_with_error_log(entry: &DirEntry, config: &Settings, _error_log: Option<&mut ErrorLog>) -> String {
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
    read_file_preview_with_error_log(path, mime_type, config, None)
}

/// Read file content for preview with error logging
fn read_file_preview_with_error_log(path: &Path, mime_type: &Option<String>, config: &Settings, _error_log: Option<&mut ErrorLog>) -> io::Result<String> {
    // Check if preview is enabled for this file type
    let can_preview = mime_type
        .as_ref()
        .and_then(|mime_str| config.get_rule(mime_str))
        .map_or(false, |rule| rule.preview);

    if !can_preview {
        return Ok(String::new());
    }

    // Read file content safely with size limit (always read first chunk)
    let file = fs::File::open(path)?;
    let mut buffer = Vec::new();
    let _bytes_read = file.take(MAX_PREVIEW_SIZE).read_to_end(&mut buffer)?;

    // Check if we read a partial file
    let metadata = fs::metadata(path)?;
    let is_truncated = metadata.len() > MAX_PREVIEW_SIZE;

    // Convert to string, handling invalid UTF-8 gracefully
    match String::from_utf8(buffer) {
        Ok(mut content) => {
            if is_truncated {
                let total_size_kb = metadata.len() / 1024;
                let preview_size_kb = MAX_PREVIEW_SIZE / 1024;
                content.push_str(&format!(
                    "\n\n[... File truncated - showing first {} KB of {} KB total ...]",
                    preview_size_kb, total_size_kb
                ));
            }
            Ok(content)
        },
        Err(_) => Ok("[Binary file - preview not available]".to_string()),
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

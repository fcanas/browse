use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Configuration for file type rules including icon and preview settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileTypeRule {
    pub icon: String,
    pub preview: bool,
}

/// Configuration for MIME type handling with primary types and subtypes
#[derive(Serialize, Deserialize, Debug)]
pub struct MimeTypeConfig {
    pub primary: HashMap<String, FileTypeRule>,
    pub subtypes: HashMap<String, FileTypeRule>,
}

/// Main application settings
#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub show_hidden_files: bool,
    pub show_icons: bool,
    pub mime_types: MimeTypeConfig,
}

impl Default for Settings {
    fn default() -> Self {
        let mut primary = HashMap::new();
        primary.insert("text".to_string(), FileTypeRule { 
            icon: "📄".to_string(), 
            preview: true 
        });
        primary.insert("image".to_string(), FileTypeRule { 
            icon: "🖼️".to_string(), 
            preview: false 
        });
        primary.insert("video".to_string(), FileTypeRule { 
            icon: "🎬".to_string(), 
            preview: false 
        });
        primary.insert("audio".to_string(), FileTypeRule { 
            icon: "🎵".to_string(), 
            preview: false 
        });
        primary.insert("application".to_string(), FileTypeRule { 
            icon: "📦".to_string(), 
            preview: false 
        });

        let mut subtypes = HashMap::new();
        subtypes.insert("text/markdown".to_string(), FileTypeRule { 
            icon: "📝".to_string(), 
            preview: true 
        });
        subtypes.insert("text/x-rust".to_string(), FileTypeRule { 
            icon: "🦀".to_string(), 
            preview: true 
        });
        subtypes.insert("application/toml".to_string(), FileTypeRule { 
            icon: "🦀".to_string(), 
            preview: true 
        });
        subtypes.insert("application/x-sh".to_string(), FileTypeRule { 
            icon: "🚀".to_string(), 
            preview: true 
        });
        subtypes.insert("symlink".to_string(), FileTypeRule { 
            icon: "🔗".to_string(), 
            preview: false 
        });
        
        Self {
            show_hidden_files: false,
            show_icons: true,
            mime_types: MimeTypeConfig { primary, subtypes },
        }
    }
}

impl Settings {
    /// Get the file type rule for a given MIME type
    pub fn get_rule(&self, mime_type: &str) -> Option<&FileTypeRule> {
        // First check subtypes for exact match
        if let Some(rule) = self.mime_types.subtypes.get(mime_type) {
            return Some(rule);
        }
        
        // Then check primary types
        if let Some(primary_type) = mime_type.split('/').next() {
            if let Some(rule) = self.mime_types.primary.get(primary_type) {
                return Some(rule);
            }
        }
        
        None
    }

    /// Validate settings and fix any inconsistencies
    pub fn validate_and_fix(&mut self) -> Result<(), String> {
        // Ensure all icons are valid UTF-8 and not empty
        for rule in self.mime_types.primary.values_mut() {
            if rule.icon.is_empty() {
                rule.icon = "📄".to_string();
            }
        }
        
        for rule in self.mime_types.subtypes.values_mut() {
            if rule.icon.is_empty() {
                rule.icon = "📄".to_string();
            }
        }
        
        Ok(())
    }
}

/// Get the path to the settings file
pub fn settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".browse")
}

/// Load settings from file with proper error handling
pub fn load_settings() -> Result<Settings, Box<dyn std::error::Error>> {
    let path = settings_path();
    
    if !path.exists() {
        return Ok(Settings::default());
    }
    
    let file = fs::File::open(&path)
        .map_err(|e| format!("Failed to open settings file {:?}: {}", path, e))?;
    
    let mut settings: Settings = serde_json::from_reader(file)
        .map_err(|e| format!("Failed to parse settings file: {}", e))?;
    
    settings.validate_and_fix()
        .map_err(|e| format!("Settings validation failed: {}", e))?;
    
    Ok(settings)
}

/// Save settings to file with proper error handling
pub fn save_settings(settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let path = settings_path();
    
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }
    
    let file = fs::File::create(&path)
        .map_err(|e| format!("Failed to create settings file {:?}: {}", path, e))?;
    
    serde_json::to_writer_pretty(file, settings)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    
    Ok(())
} 
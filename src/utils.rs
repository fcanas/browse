/// Utility functions for the file browser

use chrono::{DateTime, Local};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Format file size in human-readable format
pub fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Truncate text to fit within a given width
pub fn truncate_text(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &text[..max_width - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
        assert_eq!(format_file_size(13099650252), "12.2 GB");
        assert_eq!(format_file_size(3418437208883), "3.1 TB");
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("hello", 10), "hello");
        assert_eq!(truncate_text("hello world", 8), "hello...");
        assert_eq!(truncate_text("hi", 2), "hi");
        assert_eq!(truncate_text("hello", 3), "...");
    }

    #[test]
    fn test_format_permissions() {
        // Test basic permissions
        assert_eq!(format_permissions(0o755), "rwxr-xr-x");
        assert_eq!(format_permissions(0o644), "rw-r--r--");
        assert_eq!(format_permissions(0o000), "---------");
        assert_eq!(format_permissions(0o777), "rwxrwxrwx");

        // Test specific combinations
        assert_eq!(format_permissions(0o600), "rw-------");
        assert_eq!(format_permissions(0o444), "r--r--r--");
        assert_eq!(format_permissions(0o111), "--x--x--x");
    }

    #[test]
    fn test_format_date_compact() {
        use chrono::{Local, TimeZone, Datelike};

        let now = Local::now();

        // Test today (should show time)
        let today = now;
        let formatted = format_date_compact(&today);
        assert!(formatted.contains(":"), "Today's date should show time format HH:MM");

        // Test this year (should show month/day)
        let this_year = Local.with_ymd_and_hms(now.year(), 6, 15, 12, 0, 0).unwrap();
        let formatted = format_date_compact(&this_year);
        assert!(formatted.starts_with("Jun"), "This year should show month abbreviation");

        // Test old date (should show year)
        let old_date = Local.with_ymd_and_hms(2020, 3, 15, 12, 0, 0).unwrap();
        let formatted = format_date_compact(&old_date);
        assert_eq!(formatted, "2020", "Old date should show year");
    }
}

/// Format Unix permissions as rwx string
pub fn format_permissions(mode: u32) -> String {
    let user = format!(
        "{}{}{}",
        if mode & 0o400 != 0 { "r" } else { "-" },
        if mode & 0o200 != 0 { "w" } else { "-" },
        if mode & 0o100 != 0 { "x" } else { "-" }
    );

    let group = format!(
        "{}{}{}",
        if mode & 0o040 != 0 { "r" } else { "-" },
        if mode & 0o020 != 0 { "w" } else { "-" },
        if mode & 0o010 != 0 { "x" } else { "-" }
    );

    let other = format!(
        "{}{}{}",
        if mode & 0o004 != 0 { "r" } else { "-" },
        if mode & 0o002 != 0 { "w" } else { "-" },
        if mode & 0o001 != 0 { "x" } else { "-" }
    );

    format!("{}{}{}", user, group, other)
}

/// Format a DateTime for display in compact form
pub fn format_date_compact(datetime: &DateTime<Local>) -> String {
    let now = Local::now();
    let duration = now.signed_duration_since(*datetime);

    if duration.num_days() < 1 {
        // Show time for today
        datetime.format("%H:%M").to_string()
    } else if duration.num_days() < 365 {
        // Show month and day for this year
        datetime.format("%b %d").to_string()
    } else {
        // Show year for older files
        datetime.format("%Y").to_string()
    }
}

/// Get permissions and date info for a path
pub fn get_path_info(path: &Path) -> Option<(String, String)> {
    let metadata = fs::symlink_metadata(path).ok()?;

    let permissions = format_permissions(metadata.permissions().mode());

    let date = metadata
        .modified()
        .ok()
        .map(DateTime::from)
        .map(|dt| format_date_compact(&dt))
        .unwrap_or_else(|| "????".to_string());

    Some((permissions, date))
}

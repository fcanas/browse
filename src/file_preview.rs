use crate::browser::content_width;
use crate::utils::{format_file_size, truncate_text};
use crate::file_operations::{FileDetails};

use ratatui::{
    prelude::*,
    widgets::*,
};

const SYMLINK_PREFIX_WIDTH: usize = 16; // "Symlink -> " + padding

/// Render file preview panel
pub fn render_file_preview(frame: &mut Frame, details: &FileDetails, area: Rect) {
    let chunks = Layout::vertical([Constraint::Max(8), Constraint::Min(0)]).split(area);

    let title = details
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let truncated_title = truncate_text(&title, content_width(area));

    // Metadata section
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Size: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format_file_size(details.size)),
        ]),
        Line::from(vec![
            Span::styled("Permissions: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(details.permissions.clone()),
        ]),
    ];

    if let Some(created) = details.created {
        lines.push(Line::from(vec![
            Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(created.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    if let Some(modified) = details.modified {
        lines.push(Line::from(vec![
            Span::styled("Modified: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(modified.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    if let Some(target) = &details.symlink_target {
        let target_str = target.to_string_lossy().to_string();
        let target_width = content_width(area).saturating_sub(SYMLINK_PREFIX_WIDTH);
        let truncated_target = truncate_text(&target_str, target_width);
        lines.push(Line::from(vec![
            Span::styled("Symlink -> ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(truncated_target),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("MIME Type: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(details.mime_type.as_deref().unwrap_or("unknown")),
    ]));

    let metadata_widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(truncated_title)
            .padding(Padding::uniform(1)),
    );

    // Content preview section
    let content_widget = Paragraph::new(details.content_preview.clone())
        .block(Block::default().borders(Borders::ALL).title("Preview"));

    frame.render_widget(metadata_widget, chunks[0]);
    frame.render_widget(content_widget, chunks[1]);
}

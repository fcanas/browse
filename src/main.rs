use color_eyre::Result;
use crossterm::event::{self, Event, EnableMouseCapture, DisableMouseCapture};
use crossterm::execute;
use ratatui::DefaultTerminal;
use std::io::stdout;
use std::time::Duration;

mod app;
mod browser;
mod commands;
mod config;
mod error;
mod file_operations;
mod file_preview;
mod settings;
mod tabs;
mod ui;
mod utils;

use app::App;
use config::{save_settings, DEFAULT_POLL_INTERVAL_MS};

fn main() -> Result<()> {
    color_eyre::install()?;

    // Enable mouse capture
    execute!(stdout(), EnableMouseCapture)?;

    let mut terminal = ratatui::init();
    let mut app = App::new()?;

    let result = run(&mut terminal, &mut app);

    // Save settings before exiting
    if let Err(e) = save_settings(&app.config()) {
        eprintln!("Warning: Failed to save settings: {}", e);
    }

    // Disable mouse capture and restore terminal
    execute!(stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let poll_duration = Duration::from_millis(DEFAULT_POLL_INTERVAL_MS);

    while !app.should_quit() {
        let mut layout_info = None;
        terminal.draw(|f| {
            layout_info = Some(app.render(f));
        })?;

        if let Some(info) = layout_info {
            app.set_layout_info(info);
        }

        if event::poll(poll_duration)? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_key(key)?;
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

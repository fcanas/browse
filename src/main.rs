use color_eyre::Result;
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use std::time::Duration;

mod app;
mod commands;
mod config;
mod file_operations;
mod ui;
mod utils;

use app::App;
use config::{save_settings, DEFAULT_POLL_INTERVAL_MS};

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let mut app = App::new()?;
    
    let result = run(&mut terminal, &mut app);
    
    // Save settings before exiting
    if let Err(e) = save_settings(&app.config()) {
        eprintln!("Warning: Failed to save settings: {}", e);
    }
    
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let poll_duration = Duration::from_millis(DEFAULT_POLL_INTERVAL_MS);
    
    while !app.should_quit() {
        terminal.draw(|f| app.render(f))?;
        
        if event::poll(poll_duration)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key)?;
            }
        }
    }
    Ok(())
} 
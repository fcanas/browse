use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Represents a key binding for a command
#[derive(Debug, Clone, PartialEq)]
pub enum KeyBinding {
    /// Simple key press (e.g., Up, Down, Esc)
    Key(KeyCode),
    /// Key with modifier (e.g., Ctrl+C)
    ModifiedKey(KeyCode, KeyModifiers),
    /// Character range for search functionality
    CharRange,
}

impl KeyBinding {
    /// Check if this key binding matches the given key event
    pub fn matches(&self, key: &KeyEvent) -> bool {
        match self {
            KeyBinding::Key(code) => key.code == *code && key.modifiers.is_empty(),
            KeyBinding::ModifiedKey(code, modifiers) => {
                key.code == *code && key.modifiers.contains(*modifiers)
            }
            KeyBinding::CharRange => {
                matches!(key.code, KeyCode::Char(c) if c.is_ascii_lowercase())
            }
        }
    }

    /// Get the display text for this key binding
    pub fn display_text(&self) -> String {
        match self {
            KeyBinding::Key(KeyCode::Up) => "Up".to_string(),
            KeyBinding::Key(KeyCode::Down) => "Down".to_string(),
            KeyBinding::Key(KeyCode::Left) => "Left".to_string(),
            KeyBinding::Key(KeyCode::Right) => "Right".to_string(),
            KeyBinding::Key(KeyCode::Home) => "Home".to_string(),
            KeyBinding::Key(KeyCode::End) => "End".to_string(),
            KeyBinding::Key(KeyCode::PageUp) => "PgUp".to_string(),
            KeyBinding::Key(KeyCode::PageDown) => "PgDn".to_string(),
            KeyBinding::Key(KeyCode::Esc) => "Esc".to_string(),
            KeyBinding::Key(KeyCode::Char(c)) => c.to_string(),
            KeyBinding::ModifiedKey(KeyCode::Char(c), KeyModifiers::CONTROL) => {
                format!("Ctrl+{}", c.to_uppercase())
            }
            KeyBinding::CharRange => "a-z".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    /// Convenience method to create a Ctrl+key binding
    pub fn ctrl(c: char) -> Self {
        KeyBinding::ModifiedKey(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// Convenience method to create a simple key binding
    pub fn key(code: KeyCode) -> Self {
        KeyBinding::Key(code)
    }

    /// Convenience method to create a character key binding
    pub fn char(c: char) -> Self {
        KeyBinding::Key(KeyCode::Char(c))
    }
}

/// Represents a command that can be executed
pub struct Command {
    pub key_binding: KeyBinding,
    pub description: &'static str,
    pub action: CommandAction,
}

/// The action to be performed when a command is executed
#[derive(Clone)]
pub enum CommandAction {
    Quit,
    ShowSettings,
    ClearSearch,
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    JumpToFirst,
    JumpToLast,
    JumpUpBy10,
    JumpDownBy10,
    SetAnchor,
    SearchChar,
    ShowErrorLog,
}

impl Command {
    pub fn new(key_binding: KeyBinding, description: &'static str, action: CommandAction) -> Self {
        Self {
            key_binding,
            description,
            action,
        }
    }
}

/// Registry of all available commands
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let commands = vec![
            Command::new(
                KeyBinding::ctrl('c'),
                "Quit the application",
                CommandAction::Quit,
            ),
            Command::new(
                KeyBinding::ctrl('e'),
                "Show/hide error log",
                CommandAction::ShowErrorLog,
            ),
            Command::new(
                KeyBinding::char('?'),
                "Show/hide settings panel",
                CommandAction::ShowSettings,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Esc),
                "Clear search string",
                CommandAction::ClearSearch,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Up),
                "Navigate up",
                CommandAction::NavigateUp,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Down),
                "Navigate down",
                CommandAction::NavigateDown,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Left),
                "Navigate to parent directory",
                CommandAction::NavigateLeft,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Right),
                "Navigate to selected directory",
                CommandAction::NavigateRight,
            ),
            Command::new(
                KeyBinding::key(KeyCode::Home),
                "Jump to first item",
                CommandAction::JumpToFirst,
            ),
            Command::new(
                KeyBinding::key(KeyCode::End),
                "Jump to last item",
                CommandAction::JumpToLast,
            ),
            Command::new(
                KeyBinding::key(KeyCode::PageUp),
                "Jump up by 10 items",
                CommandAction::JumpUpBy10,
            ),
            Command::new(
                KeyBinding::key(KeyCode::PageDown),
                "Jump down by 10 items",
                CommandAction::JumpDownBy10,
            ),
            Command::new(
                KeyBinding::char('.'),
                "Set selected directory as anchor",
                CommandAction::SetAnchor,
            ),
            Command::new(
                KeyBinding::CharRange,
                "Quick search by typing",
                CommandAction::SearchChar,
            ),
        ];

        Self { commands }
    }

    /// Find a command that matches the given key event
    pub fn find_command(&self, key: &KeyEvent) -> Option<&Command> {
        self.commands.iter().find(|cmd| cmd.key_binding.matches(key))
    }

    /// Get all commands for display in help
    pub fn get_display_commands(&self) -> Vec<(String, &str)> {
        let mut display_commands = Vec::new();

        // Group some commands for better display
        display_commands.push(("Up/Down".to_string(), "Navigate list"));
        display_commands.push(("Left/Right".to_string(), "Navigate directories"));
        display_commands.push(("Home/End".to_string(), "Jump to first/last item"));
        display_commands.push(("PgUp/PgDn".to_string(), "Jump by 10 items"));

        // Add individual commands that don't need grouping
        for cmd in &self.commands {
            match &cmd.action {
                CommandAction::NavigateUp | CommandAction::NavigateDown |
                CommandAction::NavigateLeft | CommandAction::NavigateRight |
                CommandAction::JumpToFirst | CommandAction::JumpToLast |
                CommandAction::JumpUpBy10 | CommandAction::JumpDownBy10 => {
                    // Skip these as they're already grouped above
                    continue;
                }
                _ => {
                    display_commands.push((cmd.key_binding.display_text(), cmd.description));
                }
            }
        }

        display_commands
    }
}

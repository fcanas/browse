# Browse

A TUI file browser

## Features

- Miller column navigation
- File preview - primative, text-based

## Build

```bash
cargo build --release
```

## Usage

```bash
./target/release/browse
```

## Controls

- **Ctrl+C** - Quit
- **Up/Down** - Navigate list
- **Left/Right** - Navigate directories  
- **Home/End** - Jump to first/last item
- **PgUp/PgDn** - Jump by 10 items
- **?** - Settings & help panel
- **Esc** - Clear search
- **a-z** - Quick search
- **.** - Set anchor directory

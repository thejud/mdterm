# mdterm

A terminal-based Markdown viewer written in Rust. Renders Markdown files with syntax highlighting, styled formatting, and interactive navigation.

## Features

- **Interactive TUI** — Scroll, navigate with keyboard and mouse
- **Syntax highlighting** — Code blocks highlighted with the base16-ocean.dark theme
- **Rich formatting** — Headings, bold, italic, strikethrough, lists, blockquotes, tables
- **Word wrapping** — Responsive re-wrapping on terminal resize
- **Pipe-friendly** — Outputs plain styled text when stdout is piped

## Installation

Requires Rust 1.85+ (edition 2024).

```bash
cargo install --path .
```

## Usage

```bash
mdterm README.md
```

When piped, mdterm outputs styled text without the interactive viewer:

```bash
mdterm README.md | less -R
```

### Controls

| Key                          | Action     |
|------------------------------|------------|
| `j` / `Down`                 | Scroll down |
| `k` / `Up`                   | Scroll up  |
| `Space` / `d` / `Page Down`  | Page down  |
| `b` / `u` / `Page Up`        | Page up    |
| `g` / `Home`                 | Go to top  |
| `G` / `End`                  | Go to bottom |
| `q` / `Esc` / `Ctrl+C`      | Quit       |
| Mouse scroll                 | Scroll     |

## Building

```bash
cargo build --release
```

## License

MIT

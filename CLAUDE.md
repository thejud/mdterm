# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

mdterm is a terminal-based Markdown viewer written in Rust. It renders Markdown files with syntax highlighting, styled formatting, and interactive navigation (scrolling, keyboard/mouse controls). When stdout is piped, it outputs plain styled text instead of the interactive TUI.

## Build Commands

```bash
cargo build              # debug build
cargo build --release    # release build
cargo run -- <file.md>   # run with a markdown file
cargo check              # type-check without building
cargo clippy             # lint
cargo fmt                # format code
cargo test               # run tests (none exist yet)
```

## Architecture

Four source files in `src/`:

- **main.rs** — Entry point. Parses CLI args, reads the markdown file, and dispatches to either the interactive viewer (if stdout is a TTY) or plain line output (if piped).
- **markdown.rs** — Stateful markdown renderer. Processes `pulldown-cmark` events into `Vec<Line>` (styled text lines). Handles syntax highlighting for code blocks via `syntect` (base16-ocean.dark theme). Tracks inline formatting state (bold/italic/strikethrough) and block context (headings, lists, blockquotes, code blocks).
- **style.rs** — Data types (`Style`, `StyledSpan`, `Line`) and word-wrapping logic (`wrap_lines`). Wrapping splits spans into word/whitespace segments and handles long-word character-level breaks.
- **viewer.rs** — Interactive TUI using `crossterm`. Manages raw mode, alternate screen, mouse capture, keyboard/mouse event loop, viewport scrolling, and responsive re-wrapping on terminal resize. Uses RAII (`TerminalGuard`) for terminal cleanup on panic.

**Data flow:** markdown text → `pulldown-cmark` events → `Renderer` (markdown.rs) → `Vec<Line>` → `wrap_lines` (style.rs) → terminal output (viewer.rs)

## Key Dependencies

- **pulldown-cmark 0.11** — CommonMark parser (events/AST)
- **crossterm 0.28** — Terminal control (raw mode, colors, events)
- **syntect 5** — Syntax highlighting for code blocks

## Rust Edition

Uses Rust edition 2024 (requires rustc 1.85+).

use std::io::{self, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind, read,
    },
    execute, queue,
    style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};

use crate::style::{Line, StyledSpan, wrap_lines};

/// RAII guard to restore terminal state on drop (including panics).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

struct SearchMatch {
    line: usize,
    start: usize,
    end: usize,
}

struct SearchState {
    query: String,
    input_buf: String,
    matches: Vec<SearchMatch>,
    current_idx: usize,
    input_active: bool,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            input_buf: String::new(),
            matches: Vec::new(),
            current_idx: 0,
            input_active: false,
        }
    }

    fn execute(&mut self, lines: &[Line]) {
        self.query = self.input_buf.clone();
        self.find_matches(lines);
    }

    fn find_matches(&mut self, lines: &[Line]) {
        self.matches.clear();
        self.current_idx = 0;
        if self.query.is_empty() {
            return;
        }
        let query_lower = self.query.to_lowercase();
        let qbyte_len = query_lower.len();
        let qchar_len = query_lower.chars().count();
        for (line_idx, line) in lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
            let text_lower = text.to_lowercase();
            let mut pos = 0;
            while pos < text_lower.len() {
                if let Some(found) = text_lower[pos..].find(&query_lower) {
                    let byte_start = pos + found;
                    let char_start = text_lower[..byte_start].chars().count();
                    self.matches.push(SearchMatch {
                        line: line_idx,
                        start: char_start,
                        end: char_start + qchar_len,
                    });
                    pos = byte_start + qbyte_len;
                } else {
                    break;
                }
            }
        }
    }

    fn jump_nearest(&mut self, viewport_offset: usize) {
        if let Some(idx) = self.matches.iter().position(|m| m.line >= viewport_offset) {
            self.current_idx = idx;
        } else if !self.matches.is_empty() {
            self.current_idx = 0;
        }
    }

    fn next(&mut self) {
        if !self.matches.is_empty() {
            self.current_idx = (self.current_idx + 1) % self.matches.len();
        }
    }

    fn prev(&mut self) {
        if !self.matches.is_empty() {
            self.current_idx = self
                .current_idx
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
        }
    }

    fn current_line(&self) -> Option<usize> {
        self.matches.get(self.current_idx).map(|m| m.line)
    }

    fn has_results(&self) -> bool {
        !self.query.is_empty()
    }

    fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_idx = 0;
    }

    fn highlights_for_line(&self, line_idx: usize) -> Vec<(usize, usize, bool)> {
        self.matches
            .iter()
            .enumerate()
            .filter(|(_, m)| m.line == line_idx)
            .map(|(i, m)| (m.start, m.end, i == self.current_idx))
            .collect()
    }
}

pub fn run(content: &str, filename: &str) -> io::Result<()> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
    let _guard = TerminalGuard;

    let (mut cols, mut rows) = size()?;
    let content_width = (cols as usize).saturating_sub(4);
    let lines = crate::markdown::render(content, content_width);
    let mut wrapped = wrap_lines(&lines, content_width);
    let mut offset: usize = 0;
    let mut search = SearchState::new();

    loop {
        let height = rows as usize;
        let width = cols as usize;
        let viewport = height.saturating_sub(2);
        let max_offset = wrapped.len().saturating_sub(viewport);
        offset = offset.min(max_offset);

        render_frame(
            &mut stdout,
            &wrapped,
            offset,
            width,
            viewport,
            filename,
            &search,
        )?;

        match read()? {
            Event::Key(ke) if ke.kind == KeyEventKind::Press => {
                if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                if search.input_active {
                    match ke.code {
                        KeyCode::Esc => {
                            search.input_active = false;
                            search.input_buf.clear();
                        }
                        KeyCode::Enter => {
                            search.input_active = false;
                            search.execute(&wrapped);
                            search.jump_nearest(offset);
                            scroll_to_match(&search, &mut offset, viewport, max_offset);
                        }
                        KeyCode::Backspace => {
                            search.input_buf.pop();
                        }
                        KeyCode::Char(c) => {
                            search.input_buf.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match ke.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Esc => {
                            if search.has_results() {
                                search.clear();
                            } else {
                                break;
                            }
                        }

                        KeyCode::Char('/') => {
                            search.input_active = true;
                            search.input_buf.clear();
                        }
                        KeyCode::Char('n') if search.has_results() => {
                            search.next();
                            scroll_to_match(&search, &mut offset, viewport, max_offset);
                        }
                        KeyCode::Char('N') if search.has_results() => {
                            search.prev();
                            scroll_to_match(&search, &mut offset, viewport, max_offset);
                        }

                        KeyCode::Down | KeyCode::Char('j') => {
                            offset = (offset + 1).min(max_offset);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            offset = offset.saturating_sub(1);
                        }
                        KeyCode::Char(' ') | KeyCode::PageDown | KeyCode::Char('d')
                            if ke.modifiers.contains(KeyModifiers::CONTROL)
                                || ke.code != KeyCode::Char('d') =>
                        {
                            offset = (offset + viewport).min(max_offset);
                        }
                        KeyCode::Char('b') | KeyCode::PageUp | KeyCode::Char('u')
                            if ke.modifiers.contains(KeyModifiers::CONTROL)
                                || ke.code != KeyCode::Char('u') =>
                        {
                            offset = offset.saturating_sub(viewport);
                        }
                        KeyCode::Char('g') | KeyCode::Home => {
                            offset = 0;
                        }
                        KeyCode::Char('G') | KeyCode::End => {
                            offset = max_offset;
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(me) => match me.kind {
                MouseEventKind::ScrollDown => {
                    let max_offset = wrapped.len().saturating_sub(rows as usize - 1);
                    offset = (offset + 3).min(max_offset);
                }
                MouseEventKind::ScrollUp => {
                    offset = offset.saturating_sub(3);
                }
                _ => {}
            },
            Event::Resize(c, r) => {
                cols = c;
                rows = r;
                let content_width = (cols as usize).saturating_sub(4);
                let lines = crate::markdown::render(content, content_width);
                wrapped = wrap_lines(&lines, content_width);
                if search.has_results() {
                    search.find_matches(&wrapped);
                    search.jump_nearest(offset);
                }
            }
            _ => {}
        }
    }

    // _guard Drop handles cleanup
    Ok(())
}

fn scroll_to_match(search: &SearchState, offset: &mut usize, viewport: usize, max_offset: usize) {
    if let Some(target) = search.current_line()
        && (target < *offset || target >= *offset + viewport)
    {
        *offset = target.saturating_sub(viewport / 3).min(max_offset);
    }
}

fn render_frame(
    stdout: &mut io::Stdout,
    lines: &[Line],
    offset: usize,
    width: usize,
    viewport: usize,
    filename: &str,
    search: &SearchState,
) -> io::Result<()> {
    let border_fg = Color::Rgb {
        r: 55,
        g: 58,
        b: 65,
    };
    let label_fg = Color::Rgb {
        r: 120,
        g: 125,
        b: 140,
    };
    let pos_fg = Color::Rgb {
        r: 90,
        g: 95,
        b: 110,
    };
    let content_width = width.saturating_sub(4); // │ + space + content + space + │

    // ── Top border: ╭─ filename ──...──╮ ──
    let file_label = format!(" {} ", filename);
    let file_label_len = file_label.chars().count();
    let top_fill = width.saturating_sub(3 + file_label_len);

    queue!(
        stdout,
        MoveTo(0, 0),
        SetForegroundColor(border_fg),
        Print("╭─"),
        SetForegroundColor(label_fg),
        Print(&file_label),
        SetForegroundColor(border_fg),
        Print(format!("{}╮", "─".repeat(top_fill))),
        SetAttribute(Attribute::Reset),
    )?;

    // ── Content lines with left/right borders ──
    for row in 0..viewport {
        queue!(stdout, MoveTo(0, (row + 1) as u16))?;

        // Left border
        queue!(
            stdout,
            SetForegroundColor(border_fg),
            Print("│ "),
            SetAttribute(Attribute::Reset),
        )?;

        if let Some(line) = lines.get(offset + row) {
            let highlights = search.highlights_for_line(offset + row);
            let highlighted;
            let spans: &[StyledSpan] = if highlights.is_empty() {
                &line.spans
            } else {
                highlighted = apply_search_highlights(&line.spans, &highlights);
                &highlighted
            };

            let mut col = 0;
            for span in spans {
                write_span(stdout, span)?;
                col += span.text.chars().count();
            }
            if col < content_width {
                // Only extend background when all spans share the same bg
                // (e.g. code block lines). Avoids inline code bg bleeding.
                // Check original spans, not highlighted ones.
                let common_bg = line.spans.first().and_then(|s| s.style.bg).and_then(|bg| {
                    if line.spans.iter().all(|s| s.style.bg == Some(bg)) {
                        Some(bg)
                    } else {
                        None
                    }
                });
                if let Some(bg) = common_bg {
                    queue!(
                        stdout,
                        SetBackgroundColor(bg),
                        Print(" ".repeat(content_width - col)),
                        SetAttribute(Attribute::Reset)
                    )?;
                } else {
                    queue!(stdout, Print(" ".repeat(content_width - col)))?;
                }
            }
        } else {
            queue!(stdout, Print(" ".repeat(content_width)))?;
        }

        // Right border
        queue!(
            stdout,
            SetForegroundColor(border_fg),
            Print(" │"),
            SetAttribute(Attribute::Reset),
        )?;
    }

    // ── Bottom border ──
    if search.input_active {
        render_search_bar(stdout, &search.input_buf, width, viewport, border_fg)?;
    } else if search.has_results() {
        let position = format_position(lines, offset, viewport);
        render_results_bar(
            stdout, &position, width, viewport, border_fg, pos_fg, search,
        )?;
    } else {
        render_position_bar(stdout, lines, offset, width, viewport, border_fg, pos_fg)?;
    }

    stdout.flush()
}

fn render_search_bar(
    stdout: &mut io::Stdout,
    input: &str,
    width: usize,
    viewport: usize,
    border_fg: Color,
) -> io::Result<()> {
    let search_fg = Color::Rgb {
        r: 200,
        g: 170,
        b: 80,
    };
    let search_label = format!(" /{}█ ", input);
    let search_label_len = search_label.chars().count();
    let fill = width.saturating_sub(3 + search_label_len);

    queue!(
        stdout,
        MoveTo(0, (viewport + 1) as u16),
        SetForegroundColor(border_fg),
        Print("╰─"),
        SetForegroundColor(search_fg),
        Print(&search_label),
        SetForegroundColor(border_fg),
        Print("─".repeat(fill)),
        Print("╯"),
        SetAttribute(Attribute::Reset),
    )
}

fn render_results_bar(
    stdout: &mut io::Stdout,
    position: &str,
    width: usize,
    viewport: usize,
    border_fg: Color,
    pos_fg: Color,
    search: &SearchState,
) -> io::Result<()> {
    let pos_label = format!(" {} ", position);
    let pos_label_len = pos_label.chars().count();

    let search_info = if search.matches.is_empty() {
        " no match ".to_string()
    } else {
        format!(" {}/{} ", search.current_idx + 1, search.matches.len())
    };
    let search_info_len = search_info.chars().count();

    let search_info_fg = if search.matches.is_empty() {
        Color::Rgb {
            r: 200,
            g: 80,
            b: 80,
        }
    } else {
        Color::Rgb {
            r: 200,
            g: 170,
            b: 80,
        }
    };

    let fill = width.saturating_sub(4 + search_info_len + pos_label_len);

    queue!(
        stdout,
        MoveTo(0, (viewport + 1) as u16),
        SetForegroundColor(border_fg),
        Print("╰─"),
        SetForegroundColor(search_info_fg),
        Print(&search_info),
        SetForegroundColor(border_fg),
        Print("─".repeat(fill)),
        SetForegroundColor(pos_fg),
        Print(&pos_label),
        SetForegroundColor(border_fg),
        Print("─╯"),
        SetAttribute(Attribute::Reset),
    )
}

fn render_position_bar(
    stdout: &mut io::Stdout,
    lines: &[Line],
    offset: usize,
    width: usize,
    viewport: usize,
    border_fg: Color,
    pos_fg: Color,
) -> io::Result<()> {
    let position = format_position(lines, offset, viewport);
    let pos_label = format!(" {} ", position);
    let pos_label_len = pos_label.chars().count();
    let bot_fill = width.saturating_sub(3 + pos_label_len);

    queue!(
        stdout,
        MoveTo(0, (viewport + 1) as u16),
        SetForegroundColor(border_fg),
        Print(format!("╰{}", "─".repeat(bot_fill))),
        SetForegroundColor(pos_fg),
        Print(&pos_label),
        SetForegroundColor(border_fg),
        Print("─╯"),
        SetAttribute(Attribute::Reset),
    )
}

fn format_position(lines: &[Line], offset: usize, viewport: usize) -> String {
    if lines.len() <= viewport {
        "All".to_string()
    } else if offset == 0 {
        "Top".to_string()
    } else if offset >= lines.len().saturating_sub(viewport) {
        "Bot".to_string()
    } else {
        let pct = (offset + viewport) * 100 / lines.len();
        format!("{}%", pct)
    }
}

fn apply_search_highlights(
    spans: &[StyledSpan],
    highlights: &[(usize, usize, bool)],
) -> Vec<StyledSpan> {
    let match_bg = Color::Rgb {
        r: 100,
        g: 80,
        b: 0,
    };
    let current_bg = Color::Rgb {
        r: 200,
        g: 150,
        b: 20,
    };
    let current_fg = Color::Rgb {
        r: 20,
        g: 20,
        b: 20,
    };

    let mut result = Vec::new();
    let mut char_offset = 0;

    for span in spans {
        let chars: Vec<char> = span.text.chars().collect();
        let span_len = chars.len();
        let span_start = char_offset;
        let span_end = char_offset + span_len;

        // Collect cut points from highlight boundaries that fall within this span
        let mut cuts = vec![0usize, span_len];
        for &(hs, he, _) in highlights {
            if hs > span_start && hs < span_end {
                cuts.push(hs - span_start);
            }
            if he > span_start && he < span_end {
                cuts.push(he - span_start);
            }
        }
        cuts.sort();
        cuts.dedup();

        for pair in cuts.windows(2) {
            let (local_start, local_end) = (pair[0], pair[1]);
            if local_start >= local_end {
                continue;
            }

            let text: String = chars[local_start..local_end].iter().collect();
            let abs_pos = span_start + local_start;

            let highlight = highlights
                .iter()
                .find(|(hs, he, _)| abs_pos >= *hs && abs_pos < *he);

            let mut style = span.style.clone();
            if let Some(&(_, _, is_current)) = highlight {
                if is_current {
                    style.bg = Some(current_bg);
                    style.fg = Some(current_fg);
                    style.bold = true;
                } else {
                    style.bg = Some(match_bg);
                }
            }

            result.push(StyledSpan { text, style });
        }

        char_offset = span_end;
    }

    result
}

fn write_span(stdout: &mut io::Stdout, span: &StyledSpan) -> io::Result<()> {
    let s = &span.style;

    if let Some(fg) = s.fg {
        queue!(stdout, SetForegroundColor(fg))?;
    }
    if let Some(bg) = s.bg {
        queue!(stdout, SetBackgroundColor(bg))?;
    }
    if s.bold {
        queue!(stdout, SetAttribute(Attribute::Bold))?;
    }
    if s.italic {
        queue!(stdout, SetAttribute(Attribute::Italic))?;
    }
    if s.underline {
        queue!(stdout, SetAttribute(Attribute::Underlined))?;
    }
    if s.strikethrough {
        queue!(stdout, SetAttribute(Attribute::CrossedOut))?;
    }
    if s.dim {
        queue!(stdout, SetAttribute(Attribute::Dim))?;
    }

    queue!(stdout, Print(&span.text), SetAttribute(Attribute::Reset))?;
    Ok(())
}

/// Print styled lines directly to stdout (for piped output).
pub fn print_lines(lines: &[Line]) {
    let mut stdout = io::stdout();
    for line in lines {
        for span in &line.spans {
            let _ = write_span(&mut stdout, span);
        }
        let _ = writeln!(stdout);
    }
}

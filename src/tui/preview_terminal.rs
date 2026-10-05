use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Preview terminal with incremental UTF-8 decoding for byte-oriented readers.
pub(crate) struct PreviewTerminal {
    pub(crate) vt: avt::Vt,
    pending: Vec<u8>,
}

impl PreviewTerminal {
    pub(crate) fn new(rows: u16, cols: u16, scrollback: usize) -> Self {
        Self {
            vt: avt::Vt::builder()
                .size(usize::from(cols.max(1)), usize::from(rows.max(1)))
                .scrollback_limit(scrollback)
                .build(),
            pending: Vec::new(),
        }
    }

    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        let mut offset = 0;
        while offset < self.pending.len() {
            match std::str::from_utf8(&self.pending[offset..]) {
                Ok(text) => {
                    self.vt.feed_str(text);
                    offset = self.pending.len();
                }
                Err(error) => {
                    let end = offset + error.valid_up_to();
                    // The UTF-8 decoder has validated this prefix.
                    self.vt
                        .feed_str(std::str::from_utf8(&self.pending[offset..end]).unwrap());
                    offset = end;
                    if let Some(len) = error.error_len() {
                        self.vt.feed_str("\u{fffd}");
                        offset += len;
                    } else {
                        break;
                    }
                }
            }
        }
        self.pending.drain(..offset);
    }

    pub(crate) fn finish(&mut self) {
        self.vt.feed_str(&String::from_utf8_lossy(&self.pending));
        self.pending.clear();
    }

    pub(crate) fn total_lines(&self) -> usize {
        self.vt
            .lines()
            .enumerate()
            .filter(|(_, line)| line.cells().iter().any(|cell| !cell.is_default()))
            .map(|(index, _)| index + 1)
            .last()
            .unwrap_or(0)
    }

    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer, scroll_y: usize, scroll_x: usize) {
        for (y, line) in (area.y..area.bottom()).zip(self.vt.lines().skip(scroll_y)) {
            for (x, cell) in (area.x..area.right()).zip(line.cells().iter().skip(scroll_x)) {
                if cell.width() == 0 {
                    continue;
                }
                let style = cell_style(cell.pen());
                if cell.width() == 2 && x + 1 >= area.right() {
                    buf[(x, y)].set_symbol(" ").set_style(style);
                    continue;
                }
                buf[(x, y)].set_char(cell.char()).set_style(style);
                if cell.width() == 2 {
                    buf[(x + 1, y)].set_symbol(" ").set_style(style);
                }
            }
        }
    }
}

fn cell_style(pen: &avt::Pen) -> Style {
    let color = |color| match color {
        None => Color::Reset,
        Some(avt::Color::Indexed(index)) => Color::Indexed(index),
        Some(avt::Color::RGB(rgb)) => Color::Rgb(rgb.r, rgb.g, rgb.b),
    };
    let mut style = Style::default().fg(color(pen.foreground())).bg(color(pen.background()));
    for (enabled, modifier) in [
        (pen.is_bold(), Modifier::BOLD),
        (pen.is_faint(), Modifier::DIM),
        (pen.is_italic(), Modifier::ITALIC),
        (pen.is_underline(), Modifier::UNDERLINED),
        (pen.is_strikethrough(), Modifier::CROSSED_OUT),
        (pen.is_blink(), Modifier::SLOW_BLINK),
        (pen.is_inverse(), Modifier::REVERSED),
    ] {
        if enabled {
            style = style.add_modifier(modifier);
        }
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lnm_and_split_utf8() {
        let mut terminal = PreviewTerminal::new(4, 12, 2);
        for byte in "\x1b[20hhello\n世界\x1b[20l\nx".as_bytes() {
            terminal.process(&[*byte]);
        }
        terminal.finish();
        assert_eq!(terminal.vt.text(), ["hello", "世界", "    x", ""]);
    }

    #[test]
    fn malformed_and_incomplete_utf8() {
        let mut terminal = PreviewTerminal::new(2, 12, 0);
        terminal.process(b"a\xffb\xe4");
        terminal.finish();
        assert_eq!(terminal.vt.text()[0], "a\u{fffd}b\u{fffd}");
    }

    #[test]
    fn scrollback_colors_and_wide_cells() {
        let mut terminal = PreviewTerminal::new(2, 8, 2);
        terminal.process("\x1b[20hfirst\nsecond\n\x1b[1;31;44m世界\nlast".as_bytes());
        assert_eq!(terminal.total_lines(), 4);
        let area = Rect::new(0, 0, 8, 2);
        let mut buf = Buffer::empty(area);
        terminal.render(area, &mut buf, 2, 0);
        assert_eq!(buf[(0, 0)].symbol(), "世");
        assert_eq!(buf[(2, 0)].symbol(), "界");
        assert_eq!(buf[(0, 0)].fg, Color::Indexed(1));
        assert_eq!(buf[(0, 0)].bg, Color::Indexed(4));
        assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD));
        terminal.process(b"\nnext\nend");
        assert!(terminal.vt.lines().count() <= 4);
    }
}

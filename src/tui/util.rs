use crossterm::terminal;
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use std::io::{self, Write};
#[cfg(unix)]
use std::{
    fs::OpenOptions,
    io::Read,
    os::fd::{AsFd, AsRawFd},
    os::unix::fs::OpenOptionsExt as _,
};
use unicode_display_width::is_double_width;

/// Clips a [`Line`] to at most `max_chars` characters, preserving per-span styles.
///
/// Iterates over the spans in `line` and collects characters until `max_chars`
/// is reached, splitting a span at the boundary if necessary.  The returned
/// line owns all its string data (`Line<'static>`), so it can be stored or
/// passed across lifetimes freely.
///
/// This is the shared primitive used whenever a fully-displayed item line
/// (potentially with ANSI colours or match highlights) must be clipped to the
/// character count of a single multiline sub-segment — for example, when
/// rendering the first sub-line of a `--multiline` item in both the item list
/// and the header-lines area.
pub(crate) fn clip_line_to_chars(line: Line<'_>, max_chars: usize) -> Line<'static> {
    let mut chars_seen = 0usize;
    let mut clipped: Vec<Span<'static>> = Vec::new();
    for span in line.spans {
        if chars_seen >= max_chars {
            break;
        }
        let span_chars: Vec<char> = span.content.chars().collect();
        let take = (max_chars - chars_seen).min(span_chars.len());
        let text: String = span_chars[..take].iter().collect();
        if !text.is_empty() {
            clipped.push(Span::styled(text, span.style));
        }
        chars_seen += span_chars.len();
    }
    Line::from(clipped)
}

// Directly taken from https://docs.rs/unicode-display-width/0.3.0/src/unicode_display_width/lib.rs.html#77-81
#[inline]
pub fn char_display_width(c: char) -> usize {
    if c == '\u{FE0F}' || is_double_width(c) {
        return 2;
    }
    1
}

pub fn wrap_text(input: Text, width: usize) -> Text {
    if input.width() <= width {
        return input;
    }

    let mut output = Text::default();

    for input_line in input.iter() {
        let mut current_line = Line::default();
        let mut w = 0;
        for span in &input_line.spans {
            let mut curr = Span::default().style(span.style);
            let mut curr_content = String::new();
            for c in span.content.chars() {
                if w + char_display_width(c) > width {
                    // Push current span and line before wrapping
                    if !curr_content.is_empty() {
                        curr.content = curr_content.into();
                        current_line.push_span(curr);
                    }
                    output.push_line(current_line);
                    // Reset for new line
                    current_line = Line::default();
                    curr = Span::default().style(span.style);
                    curr_content = String::new();
                    w = 0;
                }
                curr_content.push(c);
                w += char_display_width(c);
            }
            // Push remaining content in current span
            if !curr_content.is_empty() {
                curr.content = curr_content.into();
                current_line.push_span(curr);
            }
        }
        // Push remaining line
        if !current_line.spans.is_empty() {
            output.push_line(current_line);
        }
    }

    output
}

/// Merges styles from right to left
/// left has higher priority
/// contrary to ratatui's `Style::patch`, this will override `Reset` with the new style if set
pub(crate) fn merge_styles(left: Style, right: Style) -> Style {
    use ratatui::style::Color::Reset;
    let mut res = Style::default();
    macro_rules! set_field {
        ($res:ident, $left:ident, $right:ident, $field:ident) => {
            if left.$field == Some(Reset) {
                $res.$field = $right.$field;
            } else if $right.$field == Some(Reset) {
                $res.$field = $left.$field;
            } else {
                $res.$field = $right.$field.or($left.$field);
            }
        };
    }

    set_field!(res, left, right, fg);
    set_field!(res, left, right, bg);
    set_field!(res, left, right, underline_color);
    res.add_modifier = left.add_modifier | right.add_modifier;

    res
}

pub(crate) fn style_span(span: &mut Span, style: Style) {
    span.style = merge_styles(style, span.style);
}
pub(crate) fn style_line(line: &mut Line, style: Style) {
    line.iter_mut().for_each(|span| style_span(span, style));
}
pub(crate) fn style_text(text: &mut Text, style: Style) {
    text.iter_mut().for_each(|line| style_line(line, style));
}

/// Find the end of an OSC sequence (terminated by ESC \ or BEL)
pub(crate) fn find_osc_end(data: &[u8]) -> Option<usize> {
    for i in 2..data.len() {
        if data[i] == b'\x07' {
            // BEL terminator
            return Some(i + 1);
        }
        if i + 1 < data.len() && data[i] == b'\x1b' && data[i + 1] == b'\\' {
            // ESC \ terminator
            return Some(i + 2);
        }
    }
    None
}

/// Find the end of a CSI sequence
pub(crate) fn find_csi_end(data: &[u8]) -> Option<usize> {
    for (i, c) in data.iter().enumerate().skip(2) {
        // CSI sequences end with a byte in the range 0x40-0x7E
        if (0x40..=0x7E).contains(c) {
            return Some(i + 1);
        }
    }
    None
}

/// Handle OSC query sequences and respond to them
pub(crate) fn handle_osc_query(seq: &[u8], writer: &mut Box<dyn std::io::Write + Send>) {
    // Check if it's a query (contains '?')
    if !seq.contains(&b'?') {
        return;
    }

    // OSC 10 ; ? - Query foreground color
    if seq.starts_with(b"\x1b]10;?") {
        let _ = writer.write_all(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\");
        let _ = writer.flush();
        trace!("responded to OSC 10 foreground color query");
    }
    // OSC 11 ; ? - Query background color
    else if seq.starts_with(b"\x1b]11;?") {
        let _ = writer.write_all(b"\x1b]11;rgb:0000/0000/0000\x1b\\");
        let _ = writer.flush();
        trace!("responded to OSC 11 background color query");
    }
    // OSC 4 ; num ; ? - Query color palette
    else if seq.starts_with(b"\x1b]4;") {
        // Extract the color number and respond with a default color
        // Format: ESC ] 4 ; num ; rgb:rr/gg/bb ST
        if let Some(idx) = seq.iter().position(|&b| b == b';')
            && let Some(idx2) = seq[idx + 1..].iter().position(|&b| b == b';')
        {
            let color_num = &seq[idx + 1..idx + 1 + idx2];
            let mut response = b"\x1b]4;".to_vec();
            response.extend_from_slice(color_num);
            response.extend_from_slice(b";rgb:8080/8080/8080\x1b\\");
            let _ = writer.write_all(&response);
            let _ = writer.flush();
            trace!("responded to OSC 4 color palette query");
        }
    }
}

/// Handle CSI query sequences and respond to them.
/// Returns true if the sequence was a query (and should be filtered out).
pub(crate) fn handle_csi_query(seq: &[u8], writer: &mut Box<dyn std::io::Write + Send>) -> bool {
    // CSI c or CSI 0 c - Primary Device Attributes (DA1)
    if seq == b"\x1b[c" || seq == b"\x1b[0c" {
        let _ = writer.write_all(b"\x1b[?1;2c");
        let _ = writer.flush();
        trace!("responded to CSI c (DA1) query");
        return true;
    }
    // CSI > c or CSI > 0 c - Secondary Device Attributes (DA2)
    else if seq == b"\x1b[>c" || seq == b"\x1b[>0c" {
        let _ = writer.write_all(b"\x1b[>0;0;0c");
        let _ = writer.flush();
        trace!("responded to CSI > c (DA2) query");
        return true;
    }
    // CSI 5 n - Device Status Report
    else if seq == b"\x1b[5n" {
        let _ = writer.write_all(b"\x1b[0n");
        let _ = writer.flush();
        trace!("responded to CSI 5 n (DSR) query");
        return true;
    }
    // CSI 6 n - Cursor Position Report
    else if seq == b"\x1b[6n" {
        let _ = writer.write_all(b"\x1b[1;1R");
        let _ = writer.flush();
        trace!("responded to CSI 6 n (CPR) query");
        return true;
    }
    // CSI ? 6 n - Extended Cursor Position Report
    else if seq.starts_with(b"\x1b[?") && seq.ends_with(b"n") {
        let _ = writer.write_all(b"\x1b[?1;1;1R");
        let _ = writer.flush();
        trace!("responded to CSI ? n (DECXCPR) query");
        return true;
    }

    false
}

struct RawMode;

impl RawMode {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Get cursor position, 1-based
#[cfg(unix)]
pub(crate) fn cursor_pos_from_tty() -> io::Result<(u16, u16)> {
    let _guard = RawMode::new()?;
    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::fcntl::OFlag::O_NONBLOCK.bits())
        .open("/dev/tty")?;
    let delimiter = b'R';
    // Where is the cursor?
    // Use `ESC [ 6 n`.
    write!(tty, "\x1B[6n")?;
    let mut buf: [u8; 32] = [0; 32];
    let mut read_pos = 0;

    let mut timeout = nix::sys::time::TimeVal::new(3, 0);
    loop {
        let mut rfds = nix::sys::select::FdSet::new();
        rfds.insert(tty.as_fd());
        match nix::sys::select::select(
            rfds.highest().unwrap().as_raw_fd() + 1,
            Some(&mut rfds),
            None,
            None,
            Some(&mut timeout),
        ) {
            Ok(0) => {
                return Err(io::Error::other("Cursor position detection timed out."));
            }
            Ok(1) => match tty.read(&mut buf[read_pos..]) {
                Ok(n) => {
                    read_pos += n;
                    if buf[read_pos - 1] == delimiter {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            },
            Err(nix::errno::Errno::EINTR) => {}
            Err(errno) => {
                return Err(io::Error::from_raw_os_error(errno as i32));
            }
            Ok(_) => unreachable!(),
        }
    }
    // The answer will look like `ESC [ Cy ; Cx R`.
    let read_str = String::from_utf8(buf[..read_pos - 1].to_owned()).unwrap();
    let beg = read_str.rfind('[').unwrap();
    let coords: String = read_str.chars().skip(beg + 1).collect();
    let mut nums = coords.split(';');
    let cy = nums.next().unwrap().parse::<u16>().unwrap();
    let cx = nums.next().unwrap().parse::<u16>().unwrap();
    Ok((cx, cy))
}

/// Get cursor position, 1-based
#[cfg(windows)]
pub(crate) fn cursor_pos_from_tty() -> io::Result<(u16, u16)> {
    let _guard = RawMode::new()?;
    crossterm::cursor::position().map(|(x, y)| (x + 1, y + 1))
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;

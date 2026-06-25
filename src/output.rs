use std::io::{self, Write};

use derive_builder::Builder;

use crate::item::MatchedItem;
use crate::options::SkimOptions;
use crate::tui::Event;
use crate::tui::event::Action;

/// Output from running skim, containing the final selection and state
#[derive(Debug)]
pub struct SkimOutput {
    /// The final event that makes skim accept/quit.
    /// Was designed to determine if skim quit or accept.
    /// Typically there are only two options: `Event::EvActAbort` | `Event::EvActAccept`
    pub final_event: Event,

    /// quick pass for judging if skim aborts.
    pub is_abort: bool,

    /// The final key that makes skim accept/quit.
    /// Note that it might be `Key::Null` if it is triggered by skim.
    pub final_key: crossterm::event::KeyEvent,

    /// The query
    pub query: String,

    /// The command query
    pub cmd: String,

    /// The selected items.
    pub selected_items: Vec<MatchedItem>,

    /// The current item
    pub current: Option<MatchedItem>,

    /// The header
    pub header: String,
}

impl SkimOutput {
    /// Serialize this output to `out` according to the CLI output options.
    ///
    /// This is the formatting half of skim's output and is intentionally
    /// independent of stdout so it can be exercised by unit tests: the binary
    /// passes a locked, buffered stdout, while tests pass a `Vec<u8>`.
    ///
    /// # Errors
    ///
    /// Returns any [`io::Error`] produced while writing to `out`.
    pub fn write_output<W: Write>(&self, out: &mut W, opts: &BinOptions) -> io::Result<()> {
        if let Some(ref output_format) = opts.output_format {
            write!(
                out,
                "{}{}",
                crate::printf(
                    output_format,
                    &opts.delimiter,
                    &opts.replstr,
                    &self.selected_items.iter(),
                    &self.current,
                    &self.query,
                    &self.cmd,
                    false
                ),
                opts.output_ending
            )?;
            return Ok(());
        }

        if opts.print_query {
            write!(out, "{}{}", self.query, opts.output_ending)?;
        }

        if opts.print_cmd {
            write!(out, "{}{}", self.cmd, opts.output_ending)?;
        }

        if opts.print_header {
            write!(out, "{}{}", self.header, opts.output_ending)?;
        }

        if opts.print_current {
            if let Some(ref current) = self.current {
                write!(out, "{}{}", current.output(), opts.output_ending)?;
            } else {
                write!(out, "{}", opts.output_ending)?;
            }
        }

        if let Event::Action(Action::Accept(Some(accept_key))) = &self.final_event {
            write!(out, "{}{}", accept_key, opts.output_ending)?;
        }

        for item in &self.selected_items {
            if opts.strip_ansi {
                write!(
                    out,
                    "{}{}",
                    crate::helper::item::strip_ansi(&item.output()).0,
                    opts.output_ending
                )?;
            } else {
                write!(out, "{}{}", item.output(), opts.output_ending)?;
            }
            if opts.print_score {
                write!(out, "{}{}", item.rank.score, opts.output_ending)?;
            }
        }

        Ok(())
    }
}

/// Options controlling how a [`SkimOutput`] is serialized to the terminal.
///
/// These mirror the CLI's output-related flags (`--print-query`, `--print0`,
/// `--print-score`, …) and are derived from [`SkimOptions`] via
/// [`BinOptions::from_opts`].
#[derive(Builder)]
#[allow(missing_docs, clippy::struct_excessive_bools)]
pub struct BinOptions {
    output_ending: String,
    print_query: bool,
    print_cmd: bool,
    print_score: bool,
    print_header: bool,
    print_current: bool,
    strip_ansi: bool,
    output_format: Option<String>,
    delimiter: regex::Regex,
    replstr: String,
}

impl BinOptions {
    /// Build the output options from the parsed [`SkimOptions`].
    #[must_use]
    pub fn from_opts(opts: &SkimOptions) -> Self {
        Self {
            print_query: opts.print_query,
            print_cmd: opts.print_cmd,
            print_score: opts.print_score,
            print_header: opts.print_header,
            print_current: opts.print_current,
            output_ending: String::from(if opts.print0 { "\0" } else { "\n" }),
            strip_ansi: opts.ansi && !opts.no_strip_ansi,
            output_format: opts.output_format.clone(),
            delimiter: opts.delimiter.clone(),
            replstr: opts.replstr.clone(),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use std::sync::Arc;

    use crossterm::event::{KeyCode, KeyEvent};

    use super::*;
    use crate::item::{MatchedItem, RankBuilder};
    use crate::{Rank, SkimItem};

    fn matched(text: &str, score: i32) -> MatchedItem {
        let item: Arc<dyn SkimItem> = Arc::new(text.to_string());
        let rank = Rank {
            score,
            ..Default::default()
        };
        MatchedItem::new(item, rank, None, &RankBuilder::default())
    }

    fn output_with(items: Vec<MatchedItem>, final_event: Event) -> SkimOutput {
        SkimOutput {
            final_event,
            is_abort: false,
            final_key: KeyEvent::new(KeyCode::Null, crossterm::event::KeyModifiers::NONE),
            query: "qry".to_string(),
            cmd: "cmd".to_string(),
            selected_items: items,
            current: None,
            header: "hdr".to_string(),
        }
    }

    fn opts() -> BinOptions {
        BinOptions::from_opts(&SkimOptions::default())
    }

    fn render(out: &SkimOutput, opts: &BinOptions) -> String {
        let mut buf = Vec::new();
        out.write_output(&mut buf, opts).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn writes_selected_items_newline_separated() {
        let out = output_with(
            vec![matched("a", 0), matched("b", 0)],
            Event::Action(Action::Accept(None)),
        );
        assert_eq!(render(&out, &opts()), "a\nb\n");
    }

    #[test]
    fn print0_uses_nul_ending() {
        let mut o = opts();
        o.output_ending = "\0".to_string();
        let out = output_with(
            vec![matched("a", 0), matched("b", 0)],
            Event::Action(Action::Accept(None)),
        );
        assert_eq!(render(&out, &o), "a\0b\0");
    }

    #[test]
    fn print_query_cmd_header_precede_items_in_order() {
        let mut o = opts();
        o.print_query = true;
        o.print_cmd = true;
        o.print_header = true;
        let out = output_with(vec![matched("a", 0)], Event::Action(Action::Accept(None)));
        assert_eq!(render(&out, &o), "qry\ncmd\nhdr\na\n");
    }

    #[test]
    fn print_current_writes_blank_line_when_no_current() {
        let mut o = opts();
        o.print_current = true;
        let out = output_with(vec![matched("a", 0)], Event::Action(Action::Accept(None)));
        // No current item → a bare ending, then the selected item.
        assert_eq!(render(&out, &o), "\na\n");
    }

    #[test]
    fn print_current_writes_current_item() {
        let mut o = opts();
        o.print_current = true;
        let mut out = output_with(vec![matched("a", 0)], Event::Action(Action::Accept(None)));
        out.current = Some(matched("cur", 0));
        assert_eq!(render(&out, &o), "cur\na\n");
    }

    #[test]
    fn accept_key_is_written_before_items() {
        let out = output_with(
            vec![matched("a", 0)],
            Event::Action(Action::Accept(Some("ctrl-x".to_string()))),
        );
        assert_eq!(render(&out, &opts()), "ctrl-x\na\n");
    }

    #[test]
    fn print_score_follows_each_item() {
        let mut o = opts();
        o.print_score = true;
        let out = output_with(
            vec![matched("a", 50), matched("b", 18)],
            Event::Action(Action::Accept(None)),
        );
        assert_eq!(render(&out, &o), "a\n50\nb\n18\n");
    }

    #[test]
    fn strip_ansi_removes_escape_sequences_from_items() {
        let mut o = opts();
        o.strip_ansi = true;
        let out = output_with(
            vec![matched("\x1b[31mred\x1b[0m", 0)],
            Event::Action(Action::Accept(None)),
        );
        assert_eq!(render(&out, &o), "red\n");
    }

    #[test]
    fn strip_ansi_keeps_nul_bytes_in_item_output() {
        // NUL is not an ANSI escape, so it survives ANSI stripping (matches the
        // `--ansi` "a\0b" passthrough behavior).
        let mut o = opts();
        o.strip_ansi = true;
        let out = output_with(vec![matched("a\0b", 0)], Event::Action(Action::Accept(None)));
        assert_eq!(render(&out, &o), "a\0b\n");
    }

    #[test]
    fn no_strip_ansi_keeps_escape_sequences_in_output() {
        let o = opts(); // strip_ansi defaults to false
        assert!(!o.strip_ansi);
        let out = output_with(
            vec![matched("\x1b[31mred\x1b[0m", 0)],
            Event::Action(Action::Accept(None)),
        );
        assert_eq!(render(&out, &o), "\x1b[31mred\x1b[0m\n");
    }

    #[test]
    fn output_format_overrides_default_serialization() {
        let mut o = opts();
        // `{}` (the default replstr) expands to the current item via printf, and the
        // default per-item serialization is bypassed entirely.
        o.output_format = Some("[{}]".to_string());
        let mut out = output_with(
            vec![matched("a", 0), matched("b", 0)],
            Event::Action(Action::Accept(None)),
        );
        out.current = Some(matched("cur", 0));
        assert_eq!(render(&out, &o), "[cur]\n");
    }

    #[test]
    fn bin_options_reflect_flags() {
        let mut opts = SkimOptions::default();
        opts.print_query = true;
        opts.print0 = true;
        opts.ansi = true;
        opts.no_strip_ansi = false;
        let bin = BinOptions::from_opts(&opts);
        assert!(bin.print_query);
        assert_eq!(bin.output_ending, "\0");
        assert!(bin.strip_ansi);
    }

    #[test]
    fn bin_options_strip_ansi_requires_ansi_and_not_no_strip() {
        let mut opts = SkimOptions::default();
        opts.ansi = true;
        opts.no_strip_ansi = true;
        assert!(!BinOptions::from_opts(&opts).strip_ansi);

        opts.no_strip_ansi = false;
        assert!(BinOptions::from_opts(&opts).strip_ansi);

        opts.ansi = false;
        assert!(!BinOptions::from_opts(&opts).strip_ansi);
    }
}

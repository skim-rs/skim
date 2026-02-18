use crate::SkimItem;
use crate::field::FieldRange;
use crate::field::get_string_by_field;
use crate::helper::item::strip_ansi;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::prelude::v1::*;
use std::sync::Arc;

#[cfg(feature = "cli")]
/// Unescape a delimiter string to handle escape sequences like \x00, \t, \n, etc.
///
/// Supported escape sequences:
/// - `\x00` - `\xff`: hexadecimal byte values
/// - `\t`: tab
/// - `\n`: newline
/// - `\r`: carriage return
/// - `\\`: backslash
///
/// # Examples
///
/// ```ignore
/// use skim::util::unescape_delimiter;
///
/// assert_eq!(unescape_delimiter(r"\x00"), "\0");
/// assert_eq!(unescape_delimiter(r"\t"), "\t");
/// assert_eq!(unescape_delimiter(r"\n"), "\n");
/// assert_eq!(unescape_delimiter(r"\\"), "\\");
/// ```
pub fn unescape_delimiter(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('x') => {
                    // Handle \xNN hex escape
                    let hex: String = chars.by_ref().take(2).collect();
                    if hex.len() == 2 {
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            // For null byte and other non-UTF8 safe bytes, we need to handle carefully
                            // Regex works with strings, so we push the byte as a char
                            result.push(byte as char);
                        } else {
                            // Invalid hex, keep as literal
                            result.push('\\');
                            result.push('x');
                            result.push_str(&hex);
                        }
                    } else {
                        // Not enough hex digits
                        result.push('\\');
                        result.push('x');
                        result.push_str(&hex);
                    }
                }
                Some('t') => result.push('\t'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    // Unknown escape, keep both backslash and character
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

pub fn read_file_lines(filename: &str) -> std::result::Result<Vec<String>, std::io::Error> {
    let file = File::open(filename)?;
    BufReader::new(file).lines().collect()
}

/// Replace the fields in `pattern` with the items, expanding {...} patterns
///
/// Replaces:
/// - `{}` -> currently selected item
/// - `{1..}` etc -> fields of currently selected item, whose index is `selected`
/// - `{+}` -> all selected items (multi-select)
/// - `{q}` -> current query
/// - `{cq}` -> current command query
///
#[allow(clippy::too_many_arguments)]
pub fn printf(
    pattern: &str,
    delimiter: &Regex,
    replstr: &str,
    selected: impl Iterator<Item = Arc<dyn SkimItem>> + std::clone::Clone,
    current: Option<Arc<dyn SkimItem>>,
    query: &str,
    command_query: &str,
    quote_args: bool,
) -> String {
    let escape_arg = |s: &str, quote: bool| {
        let mut res = s.replace('\0', "\\0").to_string();
        if quote && quote_args {
            res = format!("'{}'", res.replace("'", "'\\''"));
        }
        res
    };

    let item_text = current.as_ref().map(|s| strip_ansi(&s.output()).0).unwrap_or_default();
    let escaped_item = escape_arg(&item_text, true);
    let escaped_query = escape_arg(query, true);
    let escaped_cmd_query = escape_arg(command_query, true);

    // Split on replstr first
    let replstr_parts = pattern.split(replstr);
    let mut replaced_parts = Vec::new();

    // Deal with every part to expand inside

    for part in replstr_parts {
        let mut sub = part.split('{');
        let mut replaced = sub.next().unwrap_or_default().to_string();
        for s in sub {
            let mut inside = true;
            let mut content = String::new();
            for c in s.chars() {
                if inside {
                    if c == '}' {
                        match content.as_str() {
                            "" => replaced.push_str("{}"),
                            "q" => replaced.push_str(&escaped_query),
                            "cq" => replaced.push_str(&escaped_cmd_query),
                            "n" if current.as_ref().is_some() => {
                                replaced.push_str(current.as_ref().unwrap().get_index().to_string().as_str());
                            }
                            s if s == "+n" || s.starts_with("+n:") || s == "+" || s.starts_with("+:") => {
                                let is_n = s.starts_with("+n");
                                let accessor = if is_n {
                                    |i: &Arc<dyn SkimItem>| i.get_index().to_string()
                                } else {
                                    |i: &Arc<dyn SkimItem>| strip_ansi(&i.output()).0
                                };
                                let mut quote_individually = false;

                                let delim = s.rsplit_once(':').map(|x| x.1).unwrap_or_else(|| {
                                    quote_individually = quote_args;
                                    " "
                                });

                                let mut expanded = selected
                                    .clone()
                                    .map(|i| escape_arg(&accessor(&i), quote_individually))
                                    .reduce(|a: String, b| a.to_owned() + delim + b.as_str())
                                    .unwrap_or_default();
                                if expanded.is_empty() {
                                    expanded = current
                                        .as_ref()
                                        .map(|i| escape_arg(&accessor(i), quote_args))
                                        .unwrap_or_default()
                                }

                                if quote_args && !quote_individually {
                                    replaced.push_str(&format!("'{}'", expanded));
                                } else {
                                    replaced.push_str(&expanded);
                                }
                            }
                            s => {
                                if let Some(range) = FieldRange::from_str(s) {
                                    let replacement =
                                        get_string_by_field(delimiter, &item_text, &range).unwrap_or_default();
                                    replaced.push_str(&escape_arg(replacement, true));
                                } else {
                                    log::warn!("Failed to build field range from {content}");
                                    replaced.push_str(&format!("{{{s}}}"));
                                }
                            }
                        }

                        content.clear();
                        inside = false;
                    } else {
                        content.push(c);
                    }
                } else if c == '{' {
                    inside = true;
                } else {
                    replaced.push(c);
                }
            }
            // }
        }
        replaced_parts.push(replaced);
    }

    // Join back the replstr parts into the res
    replaced_parts
        .into_iter()
        .reduce(|a: String, b| a + &escaped_item + &b)
        .unwrap_or_default()
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use super::*;
    use crate::SkimItem;
    use regex::Regex;

    #[test]
    fn test_unescape_delimiter() {
        assert_eq!(unescape_delimiter(r"\x00"), "\0");
        assert_eq!(unescape_delimiter(r"\t"), "\t");
        assert_eq!(unescape_delimiter(r"\n"), "\n");
        assert_eq!(unescape_delimiter(r"\r"), "\r");
        assert_eq!(unescape_delimiter(r"\\"), "\\");
        assert_eq!(unescape_delimiter(r"\x09"), "\t");
        assert_eq!(unescape_delimiter(r"\x0a"), "\n");
        assert_eq!(unescape_delimiter(r"foo\x00bar"), "foo\0bar");
        assert_eq!(unescape_delimiter(r"[\t\n ]+"), "[\t\n ]+");
        // Invalid escape sequences should be kept as-is
        assert_eq!(unescape_delimiter(r"\xGG"), r"\xGG");
        assert_eq!(unescape_delimiter(r"\x0"), r"\x0");
    }

    #[test]
    fn test_regex_null_byte_matching() {
        use regex::Regex;

        // Test that Regex can match null bytes
        let delimiter = unescape_delimiter(r"\x00");
        let re = Regex::new(&delimiter).unwrap();
        let text = "a\x00b\x00c";

        let matches: Vec<_> = re.find_iter(text).collect();
        assert_eq!(matches.len(), 2, "Should find 2 null byte delimiters");
        assert_eq!(matches[0].start(), 1);
        assert_eq!(matches[0].end(), 2);
        assert_eq!(matches[1].start(), 3);
        assert_eq!(matches[1].end(), 4);
    }

    #[test]
    fn test_printf() {
        let pattern = "[1] {} [2] {..2} [3] {2..} [4] {+} [5] {q} [6] {cq} [7] {+:, } [8] {+n:','}";
        let items: Vec<Arc<dyn SkimItem>> = vec![
            Arc::new("item 1"),
            Arc::new("item 2"),
            Arc::new("item 3"),
            Arc::new("item 4"),
        ];
        let delimiter = Regex::new(" ").unwrap();
        assert_eq!(
            &printf(
                pattern,
                &delimiter,
                "{}",
                items.iter().cloned(),
                Some(Arc::new("item 2")),
                "query",
                "cmd query",
                true
            ),
            "[1] 'item 2' [2] 'item 2' [3] '2' [4] 'item 1' 'item 2' 'item 3' 'item 4' [5] 'query' [6] 'cmd query' [7] 'item 1, item 2, item 3, item 4' [8] '0','0','0','0'"
        );
    }
    #[test]
    fn test_printf_plus() {
        assert_eq!(
            printf(
                "{+}",
                &Regex::new(" ").unwrap(),
                "{}",
                [Arc::new("1"), Arc::new("2")]
                    .iter()
                    .map(|x| x.clone() as Arc<dyn SkimItem>),
                Some(Arc::new("1")),
                "q",
                "cq",
                true
            ),
            "'1' '2'"
        );
        assert_eq!(
            printf(
                "{+}",
                &Regex::new(" ").unwrap(),
                "{}",
                vec![].into_iter(),
                Some(Arc::new("1")),
                "q",
                "cq",
                true
            ),
            "'1'"
        );
    }
    #[test]
    fn test_printf_norec() {
        assert_eq!(
            printf(
                "{}",
                &Regex::new(" ").unwrap(),
                "{}",
                vec![].into_iter(),
                Some(Arc::new("{..2}")),
                "q",
                "cq",
                true
            ),
            "'{..2}'"
        );
    }
    #[test]
    fn test_printf_replstr() {
        assert_eq!(
            printf(
                "{} ##",
                &Regex::new(" ").unwrap(),
                "##",
                [Arc::new("1"), Arc::new("2")]
                    .iter()
                    .map(|x| x.clone() as Arc<dyn SkimItem>),
                Some(Arc::new("1")),
                "q",
                "cq",
                true
            ),
            "{} '1'"
        );
    }
}

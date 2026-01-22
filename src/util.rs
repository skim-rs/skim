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
/// ```
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
    let ret = BufReader::new(file).lines().collect();
    debug!("file content: {:?}", ret);
    ret
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
    let escape_arg = if quote_args {
        |s: &str| format!("'{}'", s.replace('\0', "\\0").replace("'", "'\\''"))
    } else {
        |s: &str| s.replace('\0', "\\0").to_string()
    };
    let item_text = current.as_ref().map(|s| strip_ansi(&s.output()).0).unwrap_or_default();
    // Replace static fields first
    let mut res = pattern.replace(replstr, &escape_arg(&item_text));

    let mut selection_str = selected
        .clone()
        .map(|i| escape_arg(&strip_ansi(&i.output()).0))
        .collect::<Vec<_>>()
        .join(" ");
    if selection_str.is_empty() {
        selection_str = escape_arg(&item_text);
    }

    res = res.replace("{+}", &selection_str);
    res = res.replace("{q}", &escape_arg(query));
    res = res.replace("{cq}", &escape_arg(command_query));
    if let Some(ref s) = current {
        res = res.replace("{n}", &format!("{}", &s.get_index()));
    }
    res = res.replace(
        "{+n}",
        &selected
            .map(|i| escape_arg(&i.get_index().to_string()))
            .fold(String::new(), |a: String, b| a.to_owned() + b.as_str() + " "),
    );

    let mut inside = false;
    let mut pattern = String::new();
    let mut replaced = String::new();
    for c in res.chars() {
        if inside {
            if c == '}' {
                if pattern.is_empty() {
                    replaced.push_str("{}");
                } else if let Some(range) = FieldRange::from_str(&pattern) {
                    let replacement = get_string_by_field(delimiter, &item_text, &range).unwrap_or_default();
                    replaced.push_str(&escape_arg(replacement));
                } else {
                    log::warn!("Failed to build field range from {pattern}");
                }

                pattern = String::new();
                inside = false;
            } else {
                pattern.push(c);
            }
        } else if c == '{' {
            inside = true;
        } else {
            replaced.push(c);
        }
    }

    replaced
}

#[cfg(test)]
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
        let pattern = "[1] {} [2] {..2} [3] {2..} [4] {+} [5] {q} [6] {cq}";
        let items: Vec<Arc<dyn SkimItem>> = vec![
            Arc::new("item 1"),
            Arc::new("item 2"),
            Arc::new("item 3"),
            Arc::new("item 4"),
        ];
        let delimiter = Regex::new(" ").unwrap();
        assert_eq!(
            printf(
                pattern,
                &delimiter,
                "{}",
                items.iter().map(|x| x.clone()),
                Some(Arc::new("item 2")),
                "query",
                "cmd query",
                true
            ),
            String::from(
                "[1] 'item 2' [2] 'item 2' [3] '2' [4] 'item 1' 'item 2' 'item 3' 'item 4' [5] 'query' [6] 'cmd query'"
            )
        );
    }
    #[test]
    fn test_printf_plus() {
        assert_eq!(
            printf(
                "{+}",
                &Regex::new(" ").unwrap(),
                "{}",
                vec![Arc::new("1"), Arc::new("2")]
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
}

use crate::SkimItem;
use crate::field::FieldRange;
use crate::field::get_string_by_field;
use crate::helper::item::strip_ansi;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::prelude::v1::*;
use std::process::Command;
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
    BufReader::new(file).lines().collect()
}

pub(crate) fn shell_command(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        let shell = std::env::var_os("COMSPEC")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "cmd.exe".into());
        let mut command = Command::new(shell);
        command.args(["/C", cmd]);
        command
    }

    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command.args(["-c", cmd]);
        command
    }
}

/// Returns the platform-specific default command used to generate the initial item list.
///
/// This is used when `SKIM_DEFAULT_COMMAND` is not set (or is set to an empty string).
pub fn platform_default_command() -> &'static str {
    if cfg!(windows) { "dir /s /b" } else { "find ." }
}

#[cfg(windows)]
fn escape_arg(a: &str) -> String {
    format!("\"{}\"", a.replace('\0', "\\0").replace('"', "\"\""))
}

#[cfg(not(windows))]
fn escape_arg(a: &str) -> String {
    format!("'{}'", a.replace('\0', "\\0").replace('\'', "'\\''"))
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
    let escape_value = |s: &str| {
        if quote_args {
            escape_arg(s)
        } else {
            s.replace('\0', "\\0")
        }
    };
    let item_text = current.as_ref().map(|s| strip_ansi(&s.output()).0).unwrap_or_default();
    let escaped_item = escape_value(item_text.as_ref());
    let escaped_query = escape_value(query);
    let escaped_cmd_query = escape_value(command_query);

    let mut selection_str = selected
        .clone()
        .map(|i| escape_value(strip_ansi(&i.output()).0.as_ref()))
        .reduce(|a, b| a + " " + &b)
        .unwrap_or_default();
    if selection_str.is_empty() {
        selection_str = escaped_item.clone();
    }

    // Split on replstr first
    let replstr_parts = pattern.split(replstr);
    let mut replaced_parts = Vec::new();

    // Deal with every part to expand inside

    for part in replstr_parts {
        let mut sub = part.split('{');
        let mut replaced = sub.next().unwrap_or_default().to_string();
        for s in sub {
            if s.starts_with("+}") {
                replaced += &selection_str;
                replaced += s.get(2..).unwrap_or_default();
            } else if s.starts_with("q}") {
                replaced += &escaped_query;
                replaced += s.get(2..).unwrap_or_default();
            } else if s.starts_with("cq}") {
                replaced += &escaped_cmd_query;
                replaced += s.get(3..).unwrap_or_default();
            } else if s.starts_with("n}") {
                if let Some(ref item) = current {
                    replaced += item.get_index().to_string().as_str();
                }
                replaced += s.get(2..).unwrap_or_default();
            } else if s.starts_with("+n}") {
                replaced += &selected
                    .clone()
                    .map(|i| {
                        let index = i.get_index().to_string();
                        escape_value(&index)
                    })
                    .fold(String::new(), |a: String, b| a.to_owned() + b.as_str() + " ");
                replaced += s.get(3..).unwrap_or_default()
            } else {
                let mut inside = true;
                let mut content = String::new();
                for c in s.chars() {
                    if inside {
                        if c == '}' {
                            if content.is_empty() {
                                replaced.push_str("{}");
                            } else if let Some(range) = FieldRange::from_str(&content) {
                                let replacement =
                                    get_string_by_field(delimiter, item_text.as_ref(), &range).unwrap_or_default();
                                replaced.push_str(&escape_value(replacement));
                            } else {
                                log::warn!("Failed to build field range from {content}");
                                replaced.push_str(&format!("{{{content}}}"));
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
            }
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
mod test {
    use super::*;
    use crate::SkimItem;
    use regex::Regex;
    use std::ffi::{OsStr, OsString};

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
        let expected = if cfg!(windows) {
            r#"[1] "item 2" [2] "item 2" [3] "2" [4] "item 1" "item 2" "item 3" "item 4" [5] "query" [6] "cmd query""#
        } else {
            "[1] 'item 2' [2] 'item 2' [3] '2' [4] 'item 1' 'item 2' 'item 3' 'item 4' [5] 'query' [6] 'cmd query'"
        };
        assert_eq!(
            &printf(
                pattern,
                &delimiter,
                "{}",
                items.iter().map(|x| x.clone()),
                Some(Arc::new("item 2")),
                "query",
                "cmd query",
                true
            ),
            expected
        );
    }

    #[test]
    fn test_printf_plus() {
        let expected_two = if cfg!(windows) { r#""1" "2""# } else { "'1' '2'" };
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
            expected_two
        );

        let expected_one = if cfg!(windows) { r#""1""# } else { "'1'" };
        assert_eq!(
            printf("{+}", &Regex::new(" ").unwrap(), "{}", vec![].into_iter(), Some(Arc::new("1")), "q", "cq", true),
            expected_one
        );
    }

    #[test]
    fn test_printf_norec() {
        let expected = if cfg!(windows) { r#""{..2}""# } else { "'{..2}'" };
        assert_eq!(printf("{}", &Regex::new(" ").unwrap(), "{}", vec![].into_iter(), Some(Arc::new("{..2}")), "q", "cq", true), expected);
    }

    #[test]
    #[cfg(windows)]
    fn test_shell_command_uses_cmd() {
        let cmd = shell_command("echo hello");
        let expected_shell = std::env::var_os("COMSPEC")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| OsString::from("cmd.exe"));

        assert_eq!(cmd.get_program(), expected_shell.as_os_str());
        let args = cmd.get_args().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(args, [OsString::from("/C"), OsString::from("echo hello")]);
        assert_ne!(cmd.get_program(), OsStr::new("sh"));
    }

    #[test]
    #[cfg(not(windows))]
    fn test_shell_command_uses_sh() {
        let cmd = shell_command("echo hello");
        assert_eq!(cmd.get_program(), OsStr::new("sh"));
        let args = cmd.get_args().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(args, [OsString::from("-c"), OsString::from("echo hello")]);
    }

    #[test]
    #[cfg(windows)]
    fn test_platform_default_command_windows() {
        assert_eq!(platform_default_command(), "dir /s /b");
    }

    #[test]
    #[cfg(not(windows))]
    fn test_platform_default_command_unix() {
        assert_eq!(platform_default_command(), "find .");
    }
}

use super::*;
use crate::Rank;
use crate::item::{MatchedItem, RankBuilder};
use regex::Regex;
use std::sync::Arc;

fn make_item(s: &'static str) -> MatchedItem {
    MatchedItem::new(Arc::new(s), Rank::default(), None, &RankBuilder::default())
}

#[test]
fn test_quote_cmd_arg() {
    for (input, expected) in [
        ("", r#"^"^""#),
        ("hello world", r#"^"hello world^""#),
        ("%PATH%", r#"^"^%PATH^%^""#),
        (r#"a"b"#, r#"^"a\^"b^""#),
        (r#"a\"b"#, r#"^"a\\\^"b^""#),
        (r"C:\some path\", r#"^"C:\some path\\^""#),
        ("&|<>()^%!", r#"^"^&^|^<^>^(^)^^^%^!^""#),
        ("café 日本語", r#"^"café 日本語^""#),
    ] {
        assert_eq!(quote_cmd_arg(input), expected, "input: {input:?}");
    }
}

#[test]
fn test_printf_joined_values_are_quoted_once() {
    let delimiter = Regex::new(" ").unwrap();
    for (pattern, items, expected) in [
        ("{+:,}", vec![make_item("a b"), make_item("c'd")], "a b,c'd"),
        ("{+1:,}", vec![make_item("a'b x"), make_item("c y")], "a'b,c"),
        ("{+:,}", vec![], "a b"),
    ] {
        assert_eq!(
            printf(
                pattern,
                &delimiter,
                "{}",
                &items.iter(),
                &Some(make_item("a b")),
                "",
                "",
                true,
            ),
            quote_shell_arg(expected),
        );
    }
}

// Run the test binary through CMD to check both CMD parsing and Windows argv parsing.
#[cfg(windows)]
#[test]
fn test_cmd_argv_round_trip() {
    const CHILD_ENV: &str = "SKIM_TEST_CMD_ARG";
    const EXE_ENV: &str = "SKIM_TEST_CMD_EXE";
    if let Ok(expected) = std::env::var(CHILD_ENV) {
        assert_eq!(std::env::args().last().unwrap(), expected);
        return;
    }

    let delimiter = Regex::new(" ").unwrap();
    let executable = std::env::current_exe().unwrap();
    let quoted_executable = quote_cmd_arg(executable.to_str().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let forwarding_wrapper = dir.path().join("forward arguments.cmd");
    let literal_wrapper = dir.path().join("wrapper.cmd");
    // Use the Unicode environment for the executable path, not the batch file's code page.
    std::fs::write(&forwarding_wrapper, format!("@\"%{EXE_ENV}%\" %*\r\n")).unwrap();

    for value in [
        "hello world",
        "a\"b",
        "a\\\"b",
        "C:\\some path\\",
        "&|<>()^%!",
        "%PATH%",
        "café 日本語",
    ] {
        let arguments = format!(
            "--exact util::test::test_cmd_argv_round_trip --skip {}",
            printf("{q}", &delimiter, "{}", &[].iter(), &None, value, "", true,),
        );
        let mut commands = vec![
            ("direct CMD", format!("{quoted_executable} {arguments}")),
            (
                "batch forwarding",
                // Batch paths use CMD quotes, not external-program argv escaping.
                format!("@\"{}\" {arguments}", forwarding_wrapper.display()),
            ),
        ];
        if value == "%PATH%" {
            // Escape literal percent signs in batch source, but keep environment references expandable.
            std::fs::write(
                &literal_wrapper,
                format!("@\"%{EXE_ENV}%\" {}\r\n", arguments.replace('%', "%%")),
            )
            .unwrap();
            commands.push(("batch literal", format!("@\"{}\"", literal_wrapper.display())));
        }
        for (context, command) in commands {
            let output = crate::shell_cmd(&command)
                .env(CHILD_ENV, value)
                .env(EXE_ENV, &executable)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "context: {context}, value: {value:?}, output: {output:?}"
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
    }
}

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
    let items = [
        make_item("item 1"),
        make_item("item 2"),
        make_item("item 3"),
        make_item("item 4"),
    ];
    let delimiter = Regex::new(" ").unwrap();
    assert_eq!(
        &printf(
            pattern,
            &delimiter,
            "{}",
            &items.iter(),
            &Some(make_item("item 2")),
            "query",
            "cmd query",
            true
        ),
        if cfg!(unix) {
            "[1] 'item 2' [2] 'item 2' [3] '2' [4] 'item 1' 'item 2' 'item 3' 'item 4' [5] 'query' [6] 'cmd query' [7] 'item 1, item 2, item 3, item 4' [8] '0'\\'','\\''0'\\'','\\''0'\\'','\\''0'"
        } else {
            r#"[1] ^"item 2^" [2] ^"item 2^" [3] ^"2^" [4] ^"item 1^" ^"item 2^" ^"item 3^" ^"item 4^" [5] ^"query^" [6] ^"cmd query^" [7] ^"item 1, item 2, item 3, item 4^" [8] ^"0','0','0','0^""#
        }
    );
}
#[test]
fn test_printf_plus() {
    assert_eq!(
        printf(
            "{+}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[make_item("1"), make_item("2")].iter(),
            &Some(make_item("1")),
            "q",
            "cq",
            true
        ),
        if cfg!(unix) { "'1' '2'" } else { r#"^"1^" ^"2^""# }
    );
    assert_eq!(
        printf(
            "{+}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[].iter(),
            &Some(make_item("1")),
            "q",
            "cq",
            true
        ),
        if cfg!(unix) { "'1'" } else { r#"^"1^""# }
    );
}
#[test]
fn test_printf_norec() {
    assert_eq!(
        printf(
            "{}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[].iter(),
            &Some(make_item("{..2}")),
            "q",
            "cq",
            true
        ),
        if cfg!(unix) { "'{..2}'" } else { r#"^"{..2}^""# }
    );
}
#[test]
fn test_printf_replstr() {
    assert_eq!(
        printf(
            "{} ##",
            &Regex::new(" ").unwrap(),
            "##",
            &[make_item("1"), make_item("2")].iter(),
            &Some(make_item("1")),
            "q",
            "cq",
            true
        ),
        if cfg!(unix) { "{} '1'" } else { r#"{} ^"1^""# }
    );
}

/// `{n}` expands to the current item's rank index.
#[test]
fn test_printf_index() {
    assert_eq!(
        printf(
            "idx={n}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[make_item("a")].iter(),
            &Some(make_item("a")),
            "q",
            "cq",
            false
        ),
        "idx=0"
    );
}

/// `{n}` with no current item leaves the placeholder verbatim.
#[test]
fn test_printf_index_no_current() {
    assert_eq!(
        printf(
            "idx={n}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[].iter(),
            &None,
            "q",
            "cq",
            false
        ),
        "idx={n}"
    );
}

/// `{+n}` joins all selected indices; `{+n:,}` uses an explicit delimiter.
#[test]
fn test_printf_plus_index() {
    let items = [make_item("a"), make_item("b")];
    assert_eq!(
        printf(
            "{+n:,}",
            &Regex::new(" ").unwrap(),
            "{}",
            &items.iter(),
            &Some(make_item("a")),
            "q",
            "cq",
            false
        ),
        "0,0"
    );
}

/// `{+n}` with no selection falls back to the current item's index.
#[test]
fn test_printf_plus_index_fallback_to_current() {
    assert_eq!(
        printf(
            "{+n}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[].iter(),
            &Some(make_item("x")),
            "q",
            "cq",
            false
        ),
        "0"
    );
}

/// `{+FIELD}` expands a field range across every selected item.
#[test]
fn test_printf_plus_field_range() {
    let items = [make_item("a b c"), make_item("d e f")];
    assert_eq!(
        printf(
            "{+2:,}",
            &Regex::new(" ").unwrap(),
            "{}",
            &items.iter(),
            &Some(make_item("a b c")),
            "q",
            "cq",
            false
        ),
        "b,e"
    );
}

/// An unparsable `{+FIELD}` range is left verbatim and logged.
#[test]
fn test_printf_plus_invalid_field() {
    assert_eq!(
        printf(
            "{+zz}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[make_item("a b")].iter(),
            &Some(make_item("a b")),
            "q",
            "cq",
            false
        ),
        "{+zz}"
    );
}

/// An unparsable single-item `{FIELD}` range is left verbatim and logged.
#[test]
fn test_printf_invalid_field() {
    assert_eq!(
        printf(
            "{zz}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[make_item("a b")].iter(),
            &Some(make_item("a b")),
            "q",
            "cq",
            false
        ),
        "{zz}"
    );
}

/// Null bytes in item text are escaped to the literal `\0` sequence.
#[test]
fn test_printf_escapes_null_byte() {
    assert_eq!(
        printf(
            "{}",
            &Regex::new(" ").unwrap(),
            "{}",
            &[make_item("a\0b")].iter(),
            &Some(make_item("a\0b")),
            "q",
            "cq",
            false
        ),
        "a\\0b"
    );
}

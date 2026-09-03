#![allow(missing_docs, clippy::pedantic)]
// Pure Zellij-harness e2e tests: they drive `sk` entirely through the terminal
// and depend on nothing OS-specific beyond the harness itself, which is
// cross-platform (see tests/common/zellij.rs). So these run on Linux, macOS and
// Windows.
#[allow(dead_code)]
#[macro_use]
mod common;
use std::io::Cursor;

use skim::prelude::*;

use common::zellij::Keys::*;

sk_test!(sk_version_long, "", &["--version"], {
  @output[0] starts_with("sk ");
});
sk_test!(sk_version_short, "", &["-V"], {
  @output[0] starts_with("sk ");
});

sk_test!(inline_clear_on_exit, @cmd "seq 1 10", &["--height=50%"], {
    @capture[0] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(inline_clear_on_exit_reverse, @cmd "seq 1 10", &["--height=50%", "--layout=reverse"], {
    @capture[*] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(inline_clear_on_exit_reverse_list, @cmd "seq 1 10", &["--height=50%", "--layout=reverse-list"], {
    @capture[*] starts_with(">");
    @keys Escape;
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(issue_1120_height_mode_clears_on_exit, @cmd "seq 1 10", &["--height=50%"], {
    @capture[0] starts_with(">");
    @keys Key('\x1b');
    @lines |l| (!l.iter().any(|line| line.starts_with(">")));
});

sk_test!(min_height_grows_inline_viewport, @cmd "for i in {1..20}; do echo min-height-item-$i; done", &["--height=20%", "--min-height=10"], {
    @lines |l| (l.iter().filter(|line| line.contains("min-height-item-")).count() >= 7);
    @keys Escape;
});

#[test]
fn library_builder_min_height_child() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("SKIM_MIN_HEIGHT_BUILDER_CHILD").is_none() {
        return Ok(());
    }

    let options = SkimOptionsBuilder::default().height("20%").min_height("10").build()?;
    let items = SkimItemReader::default().of_bufread(Cursor::new(
        (1..=20)
            .map(|i| format!("builder-min-height-item-{i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    Skim::run_with(options, Some(items))?;
    Ok(())
}

#[test]
fn library_builder_min_height_resizes_and_scrolls() -> Result<(), Box<dyn std::error::Error>> {
    let zellij = common::zellij::ZellijController::new_named("builderminheight")?;
    zellij.send_keys(&[Str("printf '\\n%.0s' {1..22}"), Enter])?;
    zellij.until(|lines| lines.first().is_some_and(|line| line.starts_with("skim$")))?;

    let test_binary = std::env::current_exe()?.to_string_lossy().replace('\\', "/");
    let test_binary = format!("'{}'", test_binary.replace('\'', "'\\''"));
    let command =
        format!("SKIM_MIN_HEIGHT_BUILDER_CHILD=1 {test_binary} --exact library_builder_min_height_child --nocapture");
    zellij.send_keys(&[Str(&command), Enter])?;
    zellij.until(|lines| {
        lines
            .iter()
            .filter(|line| line.contains("builder-min-height-item-"))
            .count()
            >= 7
    })?;
    zellij.send_keys(&[Escape])?;
    zellij.until(|lines| lines.iter().any(|line| line.contains("test result: ok")))?;
    Ok(())
}

#[test]
fn min_height_scrolls_when_cursor_is_near_terminal_bottom() -> std::io::Result<()> {
    let mut zellij = common::zellij::ZellijController::new_named("minheightscroll")?;
    zellij.send_keys(&[Str("printf '\\n%.0s' {1..22}"), Enter])?;
    zellij.until(|lines| lines.first().is_some_and(|line| line.starts_with("skim$")))?;

    zellij.start_sk(
        Some("for i in {1..20}; do echo min-height-scroll-item-$i; done"),
        &["--height=20%", "--min-height=10"],
    )?;
    zellij.until(|lines| {
        lines
            .iter()
            .filter(|line| line.contains("min-height-scroll-item-"))
            .count()
            >= 7
    })?;
    zellij.send_keys(&[Escape])?;
    Ok(())
}

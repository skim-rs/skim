//! Unit tests for [`super::SkimOptions`] — primarily the `build()` finalizer
//! and history initialization, which apply defaults and cross-option rules.

use super::*;
use crate::item::RankCriteria;
use crate::tui::statusline::InfoDisplay;

/// Helper: invoke the env-free arg merger with `prog = "sk"` and no real CLI args.
fn merge(
    options_file_content: Option<&[u8]>,
    default_options: Option<&str>,
    default_command: Option<&str>,
) -> SkimOptions {
    SkimOptions::merge_args_and_parse(
        "sk".to_string(),
        options_file_content,
        default_options,
        std::iter::empty(),
        default_command.map(str::to_string),
    )
    .expect("options should parse")
}

#[test]
fn merge_uses_skim_default_command_when_no_cmd_flag() {
    // SKIM_DEFAULT_COMMAND fills `cmd` when neither --cmd nor a pipe is given.
    let opts = merge(None, None, Some("echo hello"));
    assert_eq!(opts.cmd.as_deref(), Some("echo hello"));
}

#[test]
fn merge_falls_back_to_builtin_default_command() {
    // With SKIM_DEFAULT_COMMAND unset, the built-in default is used.
    let opts = merge(None, None, None);
    assert_eq!(opts.cmd.as_deref(), Some(crate::SKIM_DEFAULT_COMMAND));
}

#[test]
fn merge_explicit_cmd_flag_overrides_default_command() {
    // An explicit --cmd wins over SKIM_DEFAULT_COMMAND.
    let opts = SkimOptions::merge_args_and_parse(
        "sk".to_string(),
        None,
        Some("--cmd 'echo flag'"),
        std::iter::empty(),
        Some("echo env".to_string()),
    )
    .expect("options should parse");
    assert_eq!(opts.cmd.as_deref(), Some("echo flag"));
}

#[test]
fn merge_applies_skim_default_options() {
    // SKIM_DEFAULT_OPTIONS is shlex-split and merged into the args.
    let opts = merge(None, Some("--prompt 'XXX '"), None);
    assert_eq!(opts.prompt, "XXX ");
}

#[test]
fn merge_applies_options_file_and_strips_comments() {
    // A full-line `# Preview` comment and a trailing `# Preview window` comment
    // are removed; the surviving flags must still parse cleanly.
    let content = b"# Preview\n\
--preview 'echo {}'\n\
--preview-window 'left:30%' # Preview window\n\
--prompt '>> '\n";
    let opts = merge(Some(content), None, None);
    assert_eq!(opts.preview.as_deref(), Some("echo {}"));
    assert_eq!(opts.prompt, ">> ");
}

#[test]
fn merge_options_file_comment_stripper_is_not_quote_aware() {
    // Known limitation (matches historical behavior): the `#` stripper runs
    // before shlex and does not understand quotes, so a `#` inside a quoted
    // value starts a comment. `'## '` therefore collapses to `'# '`.
    let opts = merge(Some(b"--prompt '## '\n"), None, None);
    assert_eq!(opts.prompt, "# ");
}

#[test]
fn merge_precedence_cli_args_override_default_options() {
    // CLI args come last, so they win over SKIM_DEFAULT_OPTIONS.
    let opts = SkimOptions::merge_args_and_parse(
        "sk".to_string(),
        None,
        Some("--prompt 'from-env '"),
        ["--prompt".to_string(), "from-cli ".to_string()],
        None,
    )
    .expect("options should parse");
    assert_eq!(opts.prompt, "from-cli ");
}

#[test]
fn build_no_height_forces_full_height() {
    let opts = SkimOptions {
        no_height: true,
        height: String::from("40%"),
        ..Default::default()
    }
    .build();
    assert_eq!(opts.height, "100%");
}

#[test]
fn build_multiline_default_separator() {
    let opts = SkimOptions {
        multiline: Some(None),
        read0: false,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.multiline, Some(Some(String::from("\\n"))));
}

#[test]
fn build_multiline_read0_uses_newline_separator() {
    let opts = SkimOptions {
        multiline: Some(None),
        read0: true,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.multiline, Some(Some(String::from("\n"))));
}

#[test]
fn build_reverse_sets_reverse_layout() {
    let opts = SkimOptions {
        reverse: true,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.layout, TuiLayout::Reverse);
}

#[test]
fn build_no_scrollbar_clears_scrollbar() {
    let opts = SkimOptions {
        no_scrollbar: true,
        scrollbar: String::from("|"),
        ..Default::default()
    }
    .build();
    assert!(opts.scrollbar.is_empty());
}

#[test]
fn build_inline_info_sets_inline_display() {
    let opts = SkimOptions {
        inline_info: true,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.info.display, InfoDisplay::Inline);
    assert!(opts.info.separator.is_some());
}

#[test]
fn build_no_info_hides_info() {
    let opts = SkimOptions {
        no_info: true,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.info.display, InfoDisplay::Hidden);
    assert!(opts.info.separator.is_none());
}

#[test]
fn build_no_typos_disables_typos() {
    let opts = SkimOptions {
        no_typos: true,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.typos, Typos::Disabled);
}

#[test]
fn build_no_border_forces_border_off() {
    let opts = SkimOptions {
        no_border: true,
        ..Default::default()
    }
    .build();
    assert!(matches!(opts.border, BorderType::ForceOff));
}

#[test]
fn build_filter_populates_query_when_absent() {
    let opts = SkimOptions {
        filter: Some(String::from("needle")),
        query: None,
        ..Default::default()
    }
    .build();
    assert_eq!(opts.query.as_deref(), Some("needle"));
}

#[test]
fn build_filter_does_not_override_existing_query() {
    let opts = SkimOptions {
        filter: Some(String::from("needle")),
        query: Some(String::from("explicit")),
        ..Default::default()
    }
    .build();
    assert_eq!(opts.query.as_deref(), Some("explicit"));
}

#[test]
fn build_scheme_path_adjusts_tiebreak() {
    let opts = SkimOptions {
        scheme: Some(MatchScheme::Path),
        ..Default::default()
    }
    .build();
    assert!(opts.last_match);
    assert_eq!(opts.tiebreak.first(), Some(&RankCriteria::Score));
    assert!(opts.tiebreak.contains(&RankCriteria::PathName));
}

#[test]
fn build_scheme_history_prepends_index() {
    let opts = SkimOptions {
        scheme: Some(MatchScheme::History),
        ..Default::default()
    }
    .build();
    assert_eq!(opts.tiebreak.first(), Some(&RankCriteria::Index));
}

#[test]
fn build_default_keymap_is_populated() {
    let opts = SkimOptions::default().build();
    assert!(!opts.keymap.is_empty());
}

#[test]
fn init_histories_reads_files() {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let qpath = dir.join(format!("skim_opt_test_query_{pid}.txt"));
    let cpath = dir.join(format!("skim_opt_test_cmd_{pid}.txt"));
    std::fs::write(&qpath, "q1\nq2\n").unwrap();
    std::fs::write(&cpath, "c1\nc2\n").unwrap();

    let mut opts = SkimOptions {
        history_file: Some(qpath.to_string_lossy().into_owned()),
        cmd_history_file: Some(cpath.to_string_lossy().into_owned()),
        ..Default::default()
    };
    opts.init_histories();

    assert!(opts.query_history.iter().any(|l| l == "q1"));
    assert!(opts.query_history.iter().any(|l| l == "q2"));
    assert!(opts.cmd_history.iter().any(|l| l == "c1"));
    assert!(opts.cmd_history.iter().any(|l| l == "c2"));

    let _ = std::fs::remove_file(&qpath);
    let _ = std::fs::remove_file(&cpath);
}

#[test]
fn build_history_file_adds_history_keybindings() {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let qpath = dir.join(format!("skim_opt_test_histbind_{pid}.txt"));
    std::fs::write(&qpath, "old query\n").unwrap();

    let opts = SkimOptions {
        history_file: Some(qpath.to_string_lossy().into_owned()),
        ..Default::default()
    }
    .build();

    assert!(
        opts.keymap
            .contains_key(&KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL))
    );
    assert!(
        opts.keymap
            .contains_key(&KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL))
    );

    let _ = std::fs::remove_file(&qpath);
}

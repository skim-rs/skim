use super::*;

fn completions_for(sh: &Shell) -> String {
    let mut buf = Vec::new();
    generate_completions(sh, &mut buf);
    String::from_utf8(buf).expect("completion output is valid UTF-8")
}

#[test]
fn completions_bash_contains_sk() {
    let out = completions_for(&Shell::Bash);
    assert!(out.contains("sk"), "bash completion should reference 'sk'");
    assert!(
        out.contains("--query") || out.contains("query"),
        "bash completion should include --query"
    );
}

#[test]
fn completions_zsh_contains_sk() {
    let out = completions_for(&Shell::Zsh);
    assert!(out.contains("sk"), "zsh completion should reference 'sk'");
    assert!(
        out.contains("--multi") || out.contains("multi"),
        "zsh completion should include --multi"
    );
}

#[test]
fn completions_fish_contains_sk() {
    let out = completions_for(&Shell::Fish);
    assert!(out.contains("sk"), "fish completion should reference 'sk'");
}

#[test]
fn completions_nushell_is_non_empty() {
    let out = completions_for(&Shell::Nushell);
    assert!(!out.is_empty(), "nushell completion should not be empty");
}

#[test]
fn completions_elvish_is_non_empty() {
    let out = completions_for(&Shell::Elvish);
    assert!(!out.is_empty(), "elvish completion should not be empty");
}

#[test]
fn completions_powershell_is_non_empty() {
    let out = completions_for(&Shell::PowerShell);
    assert!(!out.is_empty(), "powershell completion should not be empty");
}

fn key_bindings_for(sh: &Shell) -> String {
    let mut buf = Vec::new();
    generate_key_bindings(sh, &mut buf).expect("key-bindings generation failed");
    String::from_utf8(buf).expect("key-bindings output is valid UTF-8")
}

#[test]
fn key_bindings_bash() {
    let out = key_bindings_for(&Shell::Bash);
    assert!(
        out.starts_with("# skim key bindings for bash"),
        "unexpected bash header"
    );
    assert!(out.contains("__skim_select__()"), "missing __skim_select__ function");
}

#[test]
fn key_bindings_zsh() {
    let out = key_bindings_for(&Shell::Zsh);
    assert!(out.starts_with("# skim key bindings for zsh"), "unexpected zsh header");
    for func in [
        "__skimcmd()",
        "__skim_comprun()",
        "__skim_extract_command()",
        "__skim_generic_path_completion()",
        "_skim_complete()",
        "_skim_complete_kill()",
    ] {
        assert!(out.contains(func), "missing zsh function {func}");
    }
}

#[test]
fn key_bindings_fish() {
    let out = key_bindings_for(&Shell::Fish);
    assert!(
        out.starts_with("#!/bin/fish"),
        "fish key-bindings should start with shebang"
    );
    for func in [
        "function __skimcmd",
        "function __skim_parse_commandline",
        "function __skim_get_dir",
    ] {
        assert!(out.contains(func), "missing fish function '{func}'");
    }
}

#[test]
fn key_bindings_unsupported_shells_are_empty() {
    for sh in [Shell::Elvish, Shell::Nushell, Shell::PowerShell] {
        assert!(
            key_bindings_for(&sh).is_empty(),
            "{sh:?} should produce no key-bindings output"
        );
    }
}

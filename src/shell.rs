//! Provides helpers to easily generate shell completions
use std::io::Write;

use clap::CommandFactory;

use crate::SkimOptions;

/// Available shells for completion generation
#[derive(Clone, clap::ValueEnum, PartialEq, Debug)]
pub enum Shell {
    /// Bourne Again `SHell`
    Bash,
    /// Elvish shell
    Elvish,
    /// Friendly Interactive `SHell`
    Fish,
    /// Nushell (nu)
    Nushell,
    /// `PowerShell`
    PowerShell,
    /// Zsh
    Zsh,
}

/// Generate the completion and write it to stdout
pub fn generate_completions(sh: &Shell, output: &mut impl Write) {
    use Shell::{Bash, Elvish, Fish, Nushell, PowerShell, Zsh};
    let cmd = &mut SkimOptions::command();
    let bin_name = "sk";

    if *sh == Nushell {
        clap_complete::generate(clap_complete_nushell::Nushell, cmd, bin_name, output);
    } else {
        let clap_shell: clap_complete::Shell = match sh {
            Bash => clap_complete::Shell::Bash,
            Elvish => clap_complete::Shell::Elvish,
            Fish => clap_complete::Shell::Fish,
            PowerShell => clap_complete::Shell::PowerShell,
            Zsh => clap_complete::Shell::Zsh,
            Nushell => unreachable!(),
        };
        clap_complete::generate(clap_shell, cmd, bin_name, output);
    }
}

/// Generate the key-bindings script and write it to the given writer
pub fn generate_key_bindings(sh: &Shell, output: &mut impl Write) {
    use Shell::{Bash, Fish, Zsh};
    let binds_script = match sh {
        Bash => include_str!("../shell/key-bindings.bash"),
        Zsh => include_str!("../shell/key-bindings.zsh"),
        Fish => include_str!("../shell/key-bindings.fish"),
        _ => "",
    };
    if !binds_script.is_empty() {
        let _ = output.write_all(binds_script.as_bytes());
    }
}

#[cfg(test)]
mod tests {
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
        generate_key_bindings(sh, &mut buf);
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
}

//! Provides helpers to easily generate shell completions
use clap::CommandFactory;

use crate::SkimOptions;

/// Available shells for completion generation
#[derive(Clone, clap::ValueEnum, PartialEq, Debug)]
pub enum Shell {
    /// Bourne Again SHell
    Bash,
    /// Elvish shell
    Elvish,
    /// Friendly Interactive SHell
    Fish,
    /// Nushell (nu)
    Nushell,
    /// PowerShell
    PowerShell,
    /// Zsh
    Zsh,
}

/// Generate the completion and write it to stdout
pub fn generate_completions(sh: &Shell) {
    use Shell::*;
    let output = &mut std::io::stdout();
    let cmd = &mut SkimOptions::command();
    let bin_name = "sk";

    if *sh == Nushell {
        clap_complete::generate(clap_complete_nushell::Nushell, cmd, bin_name, output)
    } else {
        let clap_shell: clap_complete::Shell = match sh {
            Bash => clap_complete::Shell::Bash,
            Elvish => clap_complete::Shell::Elvish,
            Fish => clap_complete::Shell::Fish,
            PowerShell => clap_complete::Shell::PowerShell,
            Zsh => clap_complete::Shell::Zsh,
            Nushell => unreachable!(),
        };
        clap_complete::generate(clap_shell, cmd, bin_name, output)
    }
}

/// Generate the key-bindings script and write it to stdout
pub fn generate_key_bindings(sh: &Shell) {
    use Shell::*;
    let binds_script = match sh {
        Bash => include_str!("../shell/key-bindings.bash"),
        Zsh => include_str!("../shell/key-bindings.zsh"),
        Fish => include_str!("../shell/key-bindings.fish"),
        _ => "",
    };
    if !binds_script.is_empty() {
        println!("{binds_script}");
    }
}

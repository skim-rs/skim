#![allow(clippy::pedantic, clippy::complexity)]
use std::{
    env,
    path::{Path, PathBuf},
};

use clap::CommandFactory;
use skim::options::SkimOptions;

type DynError = Box<dyn std::error::Error>;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}

fn try_main() -> Result<(), DynError> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("mangen") => mangen()?,
        Some("compgen") => compgen()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

mangen            generate the man page
compgen           generate completions for popular shells
"
    )
}

fn mangen() -> Result<(), DynError> {
    let mut buffer: Vec<u8> = Default::default();
    clap_mangen::Man::new(SkimOptions::command()).render(&mut buffer)?;
    std::fs::write(project_root().join("man").join("man1").join("sk.1"), buffer)?;

    Ok(())
}

fn compgen() -> Result<(), DynError> {
    let completions_dir = project_root().join("shell");
    std::fs::create_dir_all(completions_dir.clone())?;
    // Bash
    let path = completions_dir.clone().join("bash");
    let mut buffer: Vec<u8> = Default::default();
    clap_complete::generate(
        clap_complete::Shell::Bash,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk-completion.bash"), buffer)?;

    // Zsh
    let path = completions_dir.clone().join("zsh");
    let mut buffer: Vec<u8> = Default::default();
    clap_complete::generate(
        clap_complete::Shell::Zsh,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("_zsh"), buffer)?;
    
    // Fish
    let mut buffer: Vec<u8> = Default::default();
    let path = completions_dir.clone().join("fish");
    clap_complete::generate(
        clap_complete::Shell::Fish,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk.fish"), buffer)?;

    // Elvish
    let mut buffer: Vec<u8> = Default::default();
    let path = completions_dir.clone().join("elvish");
    clap_complete::generate(
        clap_complete::Shell::Elvish,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk.elv"), buffer)?;

    // PowerShell
    let mut buffer: Vec<u8> = Default::default();
    let path = completions_dir.clone().join("powershell");
    clap_complete::generate(
        clap_complete::Shell::PowerShell,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk-completion.ps1"), buffer)?;

    // Nushell
    let mut buffer: Vec<u8> = Default::default();
    let path = completions_dir.clone().join("nushell");
    clap_complete::generate(
        clap_complete_nushell::Nushell,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk-completion.nu"), buffer)?;
    
    // Fig
    let mut buffer: Vec<u8> = Default::default();
    let path = completions_dir.clone().join("fig");
    clap_complete::generate(
        clap_complete_fig::Fig,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(path.join("sk.ts"), buffer)?;

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

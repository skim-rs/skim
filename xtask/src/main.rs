#![allow(clippy::pedantic, clippy::complexity)]
use std::{
    env,
    path::{Path, PathBuf},
};

use clap::CommandFactory;
use skim::SkimOptions;

type DynError = Box<dyn std::error::Error>;

fn main() {
    for task in env::args().skip(1) {
        if let Err(e) = try_main(&task) {
            eprintln!("{}", e);
            std::process::exit(-1);
        }
    }
}

fn try_main(task: &str) -> Result<(), DynError> {
    match task {
        "mangen" => mangen()?,
        "compgen" => compgen()?,
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
    let mandir = project_root().join("man").join("man1");
    std::fs::create_dir_all(&mandir)?;
    std::fs::write(mandir.join("sk.1"), buffer)?;

    Ok(())
}

fn compgen() -> Result<(), DynError> {
    let completions_dir = project_root().join("shell");
    std::fs::create_dir_all(&completions_dir)?;
    // Bash
    let mut buffer: Vec<u8> = Default::default();
    clap_complete::generate(
        clap_complete::Shell::Bash,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(completions_dir.join("completion.bash"), buffer)?;

    // Zsh
    let mut buffer: Vec<u8> = Default::default();
    clap_complete::generate(
        clap_complete::Shell::Zsh,
        &mut SkimOptions::command(),
        "sk",
        &mut buffer,
    );
    std::fs::write(completions_dir.join("completion.zsh"), buffer)?;

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

#[macro_use]
pub mod insta;
#[macro_use]
#[cfg(unix)]
pub mod tmux;

#[cfg(all(unix, debug_assertions, coverage))]
pub static SK: &str =
    "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= SKIM_OPTIONS_FILE= ./target/llvm-cov-target/debug/sk";
#[cfg(all(unix, debug_assertions, not(coverage)))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= SKIM_OPTIONS_FILE= ./target/debug/sk";
#[cfg(all(unix, not(debug_assertions), coverage))]
pub static SK: &str =
    "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= SKIM_OPTIONS_FILE= ./target/llvm-cov-target/release/sk";
#[cfg(all(unix, not(debug_assertions), not(coverage)))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= SKIM_OPTIONS_FILE= ./target/release/sk";

#[cfg(all(windows, debug_assertions, coverage))]
pub static SK: &str = r".\target\llvm-cov-target\debug\sk.exe";
#[cfg(all(windows, debug_assertions, not(coverage)))]
pub static SK: &str = r".\target\debug\sk.exe";
#[cfg(all(windows, not(debug_assertions), coverage))]
pub static SK: &str = r".\target\llvm-cov-target\release\sk.exe";
#[cfg(all(windows, not(debug_assertions), not(coverage)))]
pub static SK: &str = r".\target\release\sk.exe";

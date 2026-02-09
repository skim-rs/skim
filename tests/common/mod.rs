#[macro_use]
pub mod insta;
#[macro_use]
pub mod tmux;

#[cfg(all(debug_assertions, coverage))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ./target/llvm-cov-target/debug/sk";
#[cfg(all(debug_assertions, not(coverage)))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ./target/debug/sk";
#[cfg(all(not(debug_assertions), coverage))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ./target/llvm-cov-target/release/sk";
#[cfg(all(not(debug_assertions), not(coverage)))]
pub static SK: &str = "SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= ./target/release/sk";

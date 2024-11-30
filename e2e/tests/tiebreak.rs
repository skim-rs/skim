use e2e::test_utils::Keys::*;
use e2e::test_utils::TmuxController;
use std::io::Result;

fn setup(input: &str, tiebreak: &str) -> Result<TmuxController> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some(&format!("echo -en '{input}'")),
        &[&format!("--tiebreak='{tiebreak}'")],
    )?;
    tmux.until(|l| l[0].starts_with(">"))?;
    Ok(tmux)
}

#[test]
fn tiebreak_default() -> Result<()> {
    let tmux = setup("a\\nc\\nab\\nac\\nb", "score,begin,end")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Key('b')])?;
    tmux.until(|l| l[2].starts_with("> b"))
}
#[test]
fn tiebreak_neg_score() -> Result<()> {
    let tmux = setup("a\\nb\\nc\\nab\\nac", "-score")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Key('b')])?;
    tmux.until(|l| l[2].starts_with("> ab"))
}

#[test]
fn tiebreak_index() -> Result<()> {
    let tmux = setup("a\\nc\\nab\\nac\\nb", "index,score")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Key('b')])?;
    tmux.until(|l| l[2].starts_with("> ab"))
}
#[test]
fn tiebreak_neg_index() -> Result<()> {
    let tmux = setup("a\\nb\\nc\\nab\\nac", "-index,score")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Key('b')])?;
    tmux.until(|l| l[2].starts_with("> ab"))
}

#[test]
fn tiebreak_begin() -> Result<()> {
    let tmux = setup("aaba\\nb\\nc\\naba\\nac", "begin,score")?;
    tmux.until(|l| l[2].starts_with("> aaba"))?;
    tmux.send_keys(&[Str("ba")])?;
    tmux.until(|l| l[2].starts_with("> aba"))
}
#[test]
fn tiebreak_neg_begin() -> Result<()> {
    let tmux = setup("aba\\nb\\nc\\naaba\\nac", "-begin,score")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Key('b')])?;
    tmux.until(|l| l[2].starts_with("> aaba"))
}

#[test]
fn tiebreak_end() -> Result<()> {
    let tmux = setup("aaba\\nb\\nc\\naba\\nac", "end,score")?;
    tmux.until(|l| l[2].starts_with("> aaba"))?;
    tmux.send_keys(&[Str("ba")])?;
    tmux.until(|l| l[2].starts_with("> aba"))
}
#[test]
fn tiebreak_neg_end() -> Result<()> {
    let tmux = setup("aba\\nb\\nc\\naaba\\nac", "-end,score")?;
    tmux.until(|l| l[2].starts_with("> a"))?;
    tmux.send_keys(&[Str("ba")])?;
    tmux.until(|l| l[2].starts_with("> aaba"))
}

#[test]
fn tiebreak_length() -> Result<()> {
    let tmux = setup("aaba\\nb\\nc\\naba\\nac", "length,score")?;
    tmux.until(|l| l[2].starts_with("> b"))?;
    tmux.send_keys(&[Str("ba")])?;
    tmux.until(|l| l[2].starts_with("> aba"))
}
#[test]
fn tiebreak_neg_length() -> Result<()> {
    let tmux = setup("aaba\\nb\\nc\\naba\\nac", "-length,score")?;
    tmux.until(|l| l[2].starts_with("> aaba"))?;
    tmux.send_keys(&[Key('c')])?;
    tmux.until(|l| l[2].starts_with("> ac"))
}

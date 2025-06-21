use e2e::Keys::*;
use e2e::TmuxController;
use std::io::Result;
use std::io::Write;
use tempfile::NamedTempFile;

fn setup(input: &str, opts: &[&str]) -> Result<(TmuxController, String)> {
    let tmux = TmuxController::new()?;
    let outfile = tmux.start_sk(Some(&format!("echo -n -e '{input}'")), opts)?;
    tmux.until(|l| l[0].starts_with(">"))?;
    Ok((tmux, outfile))
}

#[test]
fn opt_read0() -> Result<()> {
    let (tmux, _) = setup("a\\0b\\0c", &["--read0"])?;
    let lines = tmux.capture()?;

    assert!(lines[1].starts_with("  3/3"));
    assert_eq!(lines[2].trim(), "> a");
    assert_eq!(lines[3].trim(), "b");
    assert_eq!(lines[4].trim(), "c");

    Ok(())
}

#[test]
fn opt_print0() -> Result<()> {
    let (tmux, outfile) = setup("a\\nb\\nc", &["-m", "--print0"])?;
    tmux.send_keys(&[BTab, BTab, Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;

    let lines = tmux.output(&outfile)?;

    assert_eq!(lines, vec!["a\0b\0"]);

    Ok(())
}

#[test]
fn opt_with_nth_preview() -> Result<()> {
    let (tmux, _) = setup(
        "f1,f2,f3,f4",
        &["--delimiter", ",", "--with-nth", "2..", "--preview", "'echo X{1}Y'"],
    )?;

    tmux.until(|l| l.iter().any(|s| s.contains("Xf1Y")))?;

    Ok(())
}

#[test]
fn opt_min_query_length() -> Result<()> {
    let (tmux, _) = setup("line1\nline2\nline3", &["--min-query-length", "3"])?;

    // With empty query, no results should be shown
    let lines = tmux.capture()?;
    assert!(!lines.iter().any(|s| s.contains("line")));

    // Type 'li' (2 chars), still no results should be shown
    tmux.send_keys(&[Key('l'), Key('i')])?;
    tmux.until(|l| l[0].starts_with("> li"))?;
    let lines = tmux.capture()?;
    assert!(!lines.iter().any(|s| s.contains("line")));

    // Type 'n' (3rd char), now results should appear
    tmux.send_keys(&[Key('n')])?;
    tmux.until(|l| l[0].starts_with("> lin"))?;
    let lines = tmux.capture()?;
    assert!(lines.iter().any(|s| s.contains("line")));

    Ok(())
}

#[test]
fn opt_with_nth_1() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "1"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f1,")?;

    Ok(())
}
#[test]
fn opt_with_nth_2() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f2,")?;

    Ok(())
}
#[test]
fn opt_with_nth_4() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "4"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f4")?;

    Ok(())
}
#[test]
fn opt_with_nth_oob() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "5"])?;

    tmux.until(|l| l.len() > 2 && l[2] == ">")?;

    Ok(())
}
#[test]
fn opt_with_nth_neg_1() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-1"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f4")?;

    Ok(())
}
#[test]
fn opt_with_nth_neg_2() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-2"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f3,")?;

    Ok(())
}
#[test]
fn opt_with_nth_neg_4() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-4"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f1,")?;

    Ok(())
}
#[test]
fn opt_with_nth_neg_oob() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-5"])?;

    tmux.until(|l| l.len() > 2 && l[2] == ">")?;

    Ok(())
}
#[test]
fn opt_with_nth_range_to_end() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2.."])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f2,f3,f4")?;

    Ok(())
}
#[test]
fn opt_with_nth_range_from_start() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "..3"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,")?;

    Ok(())
}
#[test]
fn opt_with_nth_range_closed() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2..3"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> f2,f3,")?;

    Ok(())
}
#[test]
fn opt_with_nth_range_dec() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "3..2"])?;

    tmux.until(|l| l.len() > 2 && l[2] == ">")?;

    Ok(())
}

#[test]
fn opt_nth_1() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "1"])?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("2")])?;
    tmux.until(|l| l[0] == "> 2")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_2() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2"])?;

    tmux.send_keys(&[Str("2")])?;
    tmux.until(|l| l[0] == "> 2")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_4() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "4"])?;

    tmux.send_keys(&[Str("4")])?;
    tmux.until(|l| l[0] == "> 4")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_oob() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "5"])?;

    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_neg_1() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth=-1"])?;

    tmux.send_keys(&[Str("4")])?;
    tmux.until(|l| l[0] == "> 4")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_neg_2() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth=-2"])?;

    tmux.send_keys(&[Str("3")])?;
    tmux.until(|l| l[0] == "> 3")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_neg_4() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth=-4"])?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("2")])?;
    tmux.until(|l| l[0] == "> 2")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_neg_oob() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth=-5"])?;

    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_range_to_end() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2.."])?;

    tmux.send_keys(&[Str("3")])?;
    tmux.until(|l| l[0] == "> 3")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_range_from_start() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "..3"])?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("4")])?;
    tmux.until(|l| l[0] == "> 4")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}
#[test]
fn opt_nth_range_closed() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2..3"])?;

    tmux.send_keys(&[Str("2")])?;
    tmux.until(|l| l[0] == "> 2")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("3")])?;
    tmux.until(|l| l[0] == "> 3")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    tmux.send_keys(&[Ctrl(&Key('w'))])?;
    tmux.until(|l| l[0] == ">")?;

    tmux.send_keys(&[Str("4")])?;
    tmux.until(|l| l[0] == "> 4")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;
    Ok(())
}
#[test]
fn opt_nth_range_dec() -> Result<()> {
    let (tmux, _) = setup("f1,f2,f3,f4", &["--delimiter", ",", "--nth", "3..2"])?;

    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("1/1"))?;
    tmux.until(|l| l.len() > 2 && l[2] == "> f1,f2,f3,f4")?;

    tmux.send_keys(&[Str("1")])?;
    tmux.until(|l| l[0] == "> 1")?;
    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/1"))?;

    Ok(())
}

#[test]
fn opt_print_query() -> Result<()> {
    let (tmux, outfile) = setup("10\\n20\\n30", &["-q", "2", "--print-query"])?;
    tmux.send_keys(&[Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;
    let output = tmux.output(&outfile)?;

    assert_eq!(output[0], "20");
    assert_eq!(output[1], "2");

    Ok(())
}
#[test]
fn opt_print_cmd() -> Result<()> {
    let (tmux, outfile) = setup("1\\n2\\n3", &["--cmd-query", "cmd", "--print-cmd"])?;
    tmux.send_keys(&[Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;
    let output = tmux.output(&outfile)?;

    assert_eq!(output[0], "1");
    assert_eq!(output[1], "cmd");

    Ok(())
}
#[test]
fn opt_print_cmd_and_query() -> Result<()> {
    let (tmux, outfile) = setup(
        "10\\n20\\n30",
        &["--cmd-query", "cmd", "--print-cmd", "-q", "2", "--print-query"],
    )?;
    tmux.send_keys(&[Enter])?;
    tmux.until(|l| !l[0].starts_with(">"))?;
    let output = tmux.output(&outfile)?;

    assert_eq!(output[0], "20");
    assert_eq!(output[1], "cmd");
    assert_eq!(output[2], "2");

    Ok(())
}

#[test]
fn opt_hscroll_begin() -> Result<()> {
    let (tmux, _) = setup(&format!("b{}", &["a"; 1000].join("")), &["-q", "b"])?;

    tmux.until(|l| l.len() > 2 && l[2].ends_with(".."))
}
#[test]
fn opt_hscroll_middle() -> Result<()> {
    let (tmux, _) = setup(
        &format!("{}b{}", &["a"; 1000].join(""), &["a"; 1000].join("")),
        &["-q", "b"],
    )?;

    tmux.until(|l| l.len() > 2 && l[2].ends_with(".."))?;
    tmux.until(|l| l.len() > 2 && l[2].starts_with("> .."))
}
#[test]
fn opt_hscroll_end() -> Result<()> {
    let (tmux, _) = setup(&format!("{}b", &["a"; 1000].join("")), &["-q", "b"])?;

    tmux.until(|l| l.len() > 2 && l[2].starts_with("> .."))
}

#[test]
fn opt_no_hscroll() -> Result<()> {
    let (tmux, _) = setup(&format!("{}b", &["a"; 1000].join("")), &["-q", "b", "--no-hscroll"])?;

    tmux.until(|l| l.len() > 2 && !l[2].starts_with("> .."))?;
    tmux.until(|l| l.len() > 2 && l[2].ends_with(".."))
}

#[test]
fn opt_tabstop_default() -> Result<()> {
    let (tmux, _) = setup("a\\tb", &[])?;

    tmux.until(|l| l.len() > 2 && l[2].trim() == "> a       b")
}
#[test]
fn opt_tabstop_1() -> Result<()> {
    let (tmux, _) = setup("a\\tb", &["--tabstop", "1"])?;

    tmux.until(|l| l.len() > 2 && l[2].trim() == "> a b")
}
#[test]
fn opt_tabstop_3() -> Result<()> {
    let (tmux, _) = setup("aa\\tb", &["--tabstop", "3"])?;

    tmux.until(|l| l.len() > 2 && l[2].trim() == "> aa b")
}

#[test]
fn opt_info_control() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &[])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[1].starts_with("  3/3") && l[1].ends_with("0/0"))?;

    tmux.send_keys(&[Key('a')])?;
    tmux.until(|l| l[1].starts_with("  1/3") && l[1].ends_with("0/0"))
}
#[test]
fn opt_info_default() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--info", "default"])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    tmux.until(|l| l[1].starts_with("  3/3") && l[1].ends_with("0/0"))?;

    tmux.send_keys(&[Key('a')])?;
    tmux.until(|l| l[1].starts_with("  1/3") && l[1].ends_with("0/0"))
}
#[test]
fn opt_no_info() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--no-info"])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    let cap = tmux.capture()?;

    assert_eq!(cap[0], ">");
    assert_eq!(cap[1], "> a");

    Ok(())
}
#[test]
fn opt_info_hidden() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--info", "hidden"])?;

    tmux.until(|l| l[0].starts_with(">"))?;
    let cap = tmux.capture()?;

    assert_eq!(cap[0], ">");
    assert_eq!(cap[1], "> a");

    Ok(())
}
#[test]
fn opt_info_inline() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--info", "inline"])?;

    tmux.until(|l| l[0].starts_with(">   < 3/3") && l[0].ends_with("0/0"))?;

    tmux.send_keys(&[Key('a')])?;
    tmux.until(|l| l[0].starts_with("> a  < 1/3") && l[0].ends_with("0/0"))
}
#[test]
fn opt_inline_info() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--inline-info"])?;

    tmux.until(|l| l[0].starts_with(">   < 3/3") && l[0].ends_with("0/0"))?;

    tmux.send_keys(&[Key('a')])?;
    tmux.until(|l| l[0].starts_with("> a  < 1/3") && l[0].ends_with("0/0"))
}

#[test]
fn opt_header_only() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--header", "test_header"])?;

    tmux.until(|l| l.len() > 2 && l[2].trim() == "test_header")
}
#[test]
fn opt_header_inline_info() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--header", "test_header", "--inline-info"])?;

    tmux.until(|l| l.len() > 1 && l[1].trim() == "test_header")
}
#[test]
fn opt_header_reverse() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -e -n 'a\\nb\\nc'"),
        &["--header", "test_header", "--reverse"],
    )?;

    tmux.until(|l| l[l.len() - 1].starts_with(">"))?;

    tmux.until(|l| l[l.len() - 3].trim() == "test_header")
}
#[test]
fn opt_header_reverse_inline_info() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -e -n 'a\\nb\\nc'"),
        &["--header", "test_header", "--reverse", "--inline-info"],
    )?;

    tmux.until(|l| l[l.len() - 1].starts_with(">"))?;

    tmux.until(|l| l[l.len() - 2].trim() == "test_header")
}

#[test]
fn opt_header_lines_1() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--header-lines", "1"])?;

    tmux.until(|l| !l[2].starts_with(">") && l[2].trim() == "a")?;
    tmux.until(|l| l.len() > 3 && l[3].starts_with(">"))
}
#[test]
fn opt_header_lines_all() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--header-lines", "4"])?;

    let lines = tmux.capture()?;

    assert_eq!(lines[2].trim(), "a");
    assert_eq!(lines[3].trim(), "b");
    assert_eq!(lines[4].trim(), "c");

    Ok(())
}
#[test]
fn opt_header_lines_inline_info() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["--header-lines", "1", "--inline-info"])?;

    tmux.until(|l| !l[1].starts_with(">") && l[1].trim() == "a")
}
#[test]
fn opt_header_lines_reverse() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(Some("echo -e -n 'a\\nb\\nc'"), &["--header-lines", "1", "--reverse"])?;

    tmux.until(|l| l[l.len() - 1].starts_with(">"))?;

    tmux.until(|l| l[l.len() - 3].trim() == "a")?;
    tmux.until(|l| l[l.len() - 4].trim() == "> b")
}
#[test]
fn opt_header_lines_reverse_inline_info() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -e -n 'a\\nb\\nc'"),
        &["--header-lines", "1", "--reverse", "--inline-info"],
    )?;

    tmux.until(|l| l[l.len() - 1].starts_with(">"))?;

    tmux.until(|l| l[l.len() - 2].trim() == "a")?;
    tmux.until(|l| l[l.len() - 3].trim() == "> b")
}

#[test]
fn opt_reserved_options() -> Result<()> {
    let reserved_options = [
        "--extended",
        "--literal",
        "--no-mouse",
        "--cycle",
        "--hscroll-off=10",
        "--filepath-word",
        "--jump-labels=CHARS",
        "--border",
        "--inline-info",
        "--header=STR",
        "--header-lines=1",
        "--no-bold",
        "--history-size=10",
        "--sync",
        "--no-sort",
        "--select-1",
        "-1",
        "--exit-0",
        "-0",
    ];

    for option in reserved_options {
        println!("Starting sk with opt {}", option);
        setup("a\\nb", &[option])?;
    }

    Ok(())
}

#[test]
fn opt_multiple_flags_basic() -> Result<()> {
    let basic_flags = [
        "--bind=ctrl-a:cancel --bind ctrl-b:cancel",
        "--expect=ctrl-a --expect=ctrl-v",
        "--tiebreak=begin --tiebreak=score",
        "--cmd asdf --cmd find",
        "--query asdf -q xyz",
        "--delimiter , --delimiter . -d ,",
        "--nth 1,2 --nth=1,3 -n 1,3",
        "--with-nth 1,2 --with-nth=1,3",
        "-I {} -I XX",
        "--color base --color light",
        "--margin 30% --margin 0",
        "--min-height 30% --min-height 10",
        "--height 30% --height 10",
        "--preview 'ls {}' --preview 'cat {}'",
        "--preview-window up --preview-window down",
        "--multi -m",
        "--no-multi --no-multi",
        "--tac --tac",
        "--ansi --ansi",
        "--exact -e",
        "--regex --regex",
        "--literal --literal",
        "--no-mouse --no-mouse",
        "--cycle --cycle",
        "--no-hscroll --no-hscroll",
        "--filepath-word --filepath-word",
        "--border --border",
        "--inline-info --inline-info",
        "--no-bold --no-bold",
        "--print-query --print-query",
        "--print-cmd --print-cmd",
        "--print0 --print0",
        "--sync --sync",
        "--extended --extended",
        "--no-sort --no-sort",
        "--select-1 --select-1",
        "--exit-0 --exit-0",
    ];

    for cmd_flags in basic_flags {
        setup("a\\nb", &[cmd_flags])?;
    }

    Ok(())
}
#[test]
fn opt_multiple_flags_prompt() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(None, &["--prompt a", "--prompt b", "-p c"])?;

    tmux.until(|l| l[0].starts_with("c"))?;

    Ok(())
}
#[test]
fn opt_multiple_flags_cmd_prompt() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(None, &["-i", "--cmd-prompt a", "--cmd-prompt c"])?;

    tmux.until(|l| l[0].starts_with("c"))?;

    Ok(())
}
#[test]
fn opt_multiple_flags_cmd_query() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(None, &["-i", "--cmd-query a", "--cmd-query b"])?;

    tmux.until(|l| l[0].starts_with("c> b"))?;

    Ok(())
}
#[test]
fn opt_multiple_flags_interactive() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(None, &["-i", "--interactive", "--interactive"])?;

    tmux.until(|l| l[0].starts_with("c>"))?;

    Ok(())
}
#[test]
fn opt_multiple_flags_reverse() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(None, &["--reverse", "--reverse"])?;

    tmux.until(|l| l[l.len() - 1].starts_with(">"))?;

    Ok(())
}

#[test]
fn opt_mutliple_flags_combined_expect_first() -> Result<()> {
    let (tmux, outfile) = setup("a\\nb", &["--expect", "ctrl-a,ctrl-b"])?;
    tmux.send_keys(&[Ctrl(&Key('a'))])?;
    let output = tmux.output(&outfile)?;
    assert_eq!(output[0], "a");
    assert_eq!(output[1], "ctrl-a");
    Ok(())
}
#[test]
fn opt_mutliple_flags_combined_expect_second() -> Result<()> {
    let (tmux, outfile) = setup("a\\nb", &["--expect", "ctrl-a,ctrl-b"])?;
    tmux.send_keys(&[Ctrl(&Key('b'))])?;
    let output = tmux.output(&outfile)?;
    assert_eq!(output[0], "a");
    assert_eq!(output[1], "ctrl-b");
    Ok(())
}

#[test]
fn opt_multiple_flags_combined_nth() -> Result<()> {
    let (tmux, _) = setup("a b c\\nd e f", &["--nth 1,2"])?;

    tmux.send_keys(&[Key('c')])?;
    tmux.until(|l| l.len() > 1 && l[1].contains("0/2"))
}
#[test]
fn opt_multiple_flags_combined_with_nth() -> Result<()> {
    let (tmux, _) = setup("a b c\\nd e f", &["--with-nth 1,2"])?;

    tmux.until(|l| l.len() > 2 && l[2].ends_with("a b") && l[3].ends_with("d e"))
}

#[test]
fn opt_ansi_null() -> Result<()> {
    let (tmux, outfile) = setup("a\\0b", &["--ansi"])?;

    tmux.send_keys(&[Enter])?;

    let output = tmux.output(&outfile)?;
    println!("{:?}", output[0].as_bytes());
    assert_eq!(output[0].as_bytes(), &[97, 0, 98]);
    Ok(())
}

#[test]
fn opt_skip_to_pattern() -> Result<()> {
    let (tmux, _) = setup("a/b/c", &["--skip-to-pattern", "'[^/]*$'"])?;

    tmux.until(|l| l.len() > 2 && l[2] == "> ..c")
}

#[test]
fn opt_multi() -> Result<()> {
    let (tmux, outfile) = setup("a\\nb\\nc", &["--multi"])?;

    tmux.send_keys(&[BTab, BTab])?;
    tmux.until(|l| l.len() > 2 && l[2] == " >a" && l[3] == " >b")?;
    tmux.send_keys(&[Enter])?;

    let output = tmux.output(&outfile)?;

    assert_eq!(output[0], "b");
    assert_eq!(output[1], "a");

    Ok(())
}

#[test]
fn opt_pre_select_n() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["-m", "--pre-select-n", "2"])?;
    tmux.until(|l| l.len() > 2 && l[2] == ">>a" && l[3] == " >b")
}

#[test]
fn opt_pre_select_items() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["-m", "--pre-select-items", "$'b\\nc'"])?;
    tmux.until(|l| l.len() > 2 && l[2] == "> a" && l[3].trim() == ">b" && l[4].trim() == ">c")
}

#[test]
fn opt_pre_select_pat() -> Result<()> {
    let (tmux, _) = setup("a\\nb\\nc", &["-m", "--pre-select-pat", "'[b|c]'"])?;
    tmux.until(|l| l.len() > 2 && l[2] == "> a" && l[3].trim() == ">b" && l[4].trim() == ">c")
}

#[test]
fn opt_pre_select_file() -> Result<()> {
    let mut pre_select_file = NamedTempFile::new()?;
    pre_select_file.write(b"b\nc")?;
    let (tmux, _) = setup(
        "a\\nb\\nc",
        &["-m", "--pre-select-file", pre_select_file.path().to_str().unwrap()],
    )?;
    tmux.until(|l| l.len() > 2 && l[2] == "> a" && l[3].trim() == ">b" && l[4].trim() == ">c")
}

#[test]
fn opt_no_clear_if_empty() -> Result<()> {
    let tmux = TmuxController::new()?;
    tmux.start_sk(
        Some("echo -ne 'a\\nb\\nc'"),
        &["-i", "--no-clear-if-empty", "-c", "'cat {}'"],
    )?;
    tmux.until(|l| l[0] == "c>")?;

    tmux.send_keys(&[Str("xxxx")])?;
    tmux.until(|l| l[0] == "c> xxxx")?;

    tmux.until(|l| l.len() > 1 && l[1].trim().starts_with("0/0"))?;
    tmux.until(|l| l.len() > 2 && l[2].trim() == "> a")?;
    tmux.until(|l| l.len() > 3 && l[3].trim() == "b")?;
    Ok(())
}

#[test]
fn opt_accept_arg() -> Result<()> {
    let (tmux, outfile) = setup("a\\nb", &["--bind", "ctrl-a:accept:hello"])?;
    tmux.send_keys(&[Ctrl(&Key('a'))])?;

    let output = tmux.output(&outfile)?;
    assert_eq!(output[0], "a");
    assert_eq!(output[1], "hello");
    Ok(())
}

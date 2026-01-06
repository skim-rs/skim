#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;
use common::TmuxController;
use std::io::Result;
use std::io::Write;
use tempfile::NamedTempFile;

fn setup(input: &str, opts: &[&str]) -> Result<TmuxController> {
    let mut tmux = TmuxController::new()?;
    tmux.start_sk(Some(&format!("echo -n -e '{input}'")), opts)?;
    tmux.until(|l| l.len() > 0 && l[0].starts_with(">"))?;
    Ok(tmux)
}

sk_test!(opt_read0, "a\\0b\\0c", &["--read0"], {
  @capture[1] starts_with("  3/3");
  @capture[2] starts_with("> a");
  @capture[3] ends_with("b");
  @capture[4] ends_with("c");
});

sk_test!(opt_print0, "a\\nb\\nc", &["-m", "--print0"], {
  @lines |l| (l.len() > 4);
  @keys BTab, BTab, Enter;
  @lines |l| (l.len() > 0 && !l[0].starts_with(">"));
  @output[0] eq("a\0b\0");
});

sk_test!(opt_with_nth_preview, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2..", "--preview", "'echo X{1}Y'"], {
  @capture[*] contains("Xf1Y");
});

sk_test!(opt_min_query_length, "line1\\nline2\\nline3", &["--min-query-length", "3"], {
  // With empty query, no results should be shown
  @capture[1] contains("0/3");

  @keys Str("li");
  @capture[0] starts_with("> li");
  @capture[1] contains("0/3");

  @keys Key('n');
  @capture[0] starts_with("> lin");
  @capture[1] contains("3/3");
  @capture[*] contains("line");
});

sk_test!(opt_min_query_length_interactive, "line1\\nline2\\nline3", &["--min-query-length", "3", "-i"], {
  // With empty query, no results should be shown
  @capture[1] contains("0/3");

  @keys Str("li");
  @capture[0] starts_with("c> li");
  @capture[1] contains("0/3");

  @keys Key('n');
  @capture[0] starts_with("c> lin");
  @capture[1] contains("3/3");
  @capture[*] contains("line");
});

sk_test!(opt_with_nth_1, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "1"], {
  @capture[2] eq("> f1,");
});
sk_test!(opt_with_nth_2, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2"], {
  @capture[2] eq("> f2,");
});
sk_test!(opt_with_nth_4, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "4"], {
  @capture[2] eq("> f4");
});
sk_test!(opt_with_nth_oob, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "5"], {
  @capture[2] eq(">");
});

sk_test!(opt_with_nth_neg_1, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-1"], {
  @capture[2] eq("> f4");
});
sk_test!(opt_with_nth_neg_2, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-2"], {
  @capture[2] eq("> f3,");
});
sk_test!(opt_with_nth_neg_4, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-4"], {
  @capture[2] eq("> f1,");
});
sk_test!(opt_with_nth_oob_4, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth=-5"], {
  @capture[2] eq(">");
});
sk_test!(opt_with_nth_range_to_end, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2.."], {
  @capture[2] eq("> f2,f3,f4");
});
sk_test!(opt_with_nth_range_from_start, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "..3"], {
  @capture[2] eq("> f1,f2,f3,");
});
sk_test!(opt_with_nth_range_closed, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "2..3"], {
  @capture[2] eq("> f2,f3,");
});
sk_test!(opt_with_nth_range_desc, "f1,f2,f3,f4", &["--delimiter", ",", "--with-nth", "3..2"], {
  @capture[2] eq(">");
});

sk_test!(opt_nth_1, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "1"], {
  @keys Key('1');
  @capture[0] eq("> 1");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("2");
  @capture[0] eq("> 2");
  @capture[1] contains("0/1");
});
sk_test!(opt_nth_2, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2"], {
  @keys Str("2");
  @capture[0] eq("> 2");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_4, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "4"], {
  @keys Str("4");
  @capture[0] eq("> 4");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_oob, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "5"], {
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});
sk_test!(opt_nth_neg_1, "f1,f2,f3,f4", &["--delimiter", ",", "--nth=-1"], {
  @keys Str("4");
  @capture[0] eq("> 4");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_neg_2, "f1,f2,f3,f4", &["--delimiter", ",", "--nth=-2"], {
  @keys Str("3");
  @capture[0] eq("> 3");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_neg_4, "f1,f2,f3,f4", &["--delimiter", ",", "--nth=-4"], {
  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("2");
  @capture[0] eq("> 2");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_neg_oob, "f1,f2,f3,f4", &["--delimiter", ",", "--nth=-5"], {
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});
sk_test!(opt_nth_range_to_end, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2.."], {
  @keys Str("3");
  @capture[0] eq("> 3");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_range_from_start, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "..3"], {
  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("4");
  @capture[0] eq("> 4");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_range_closed, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "2..3"], {
  @keys Str("2");
  @capture[0] eq("> 2");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("3");
  @capture[0] eq("> 3");
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");

  @keys Ctrl(&Key('w'));
  @capture[0] eq(">");

  @keys Str("4");
  @capture[0] eq("> 4");
  @capture[1] contains("0/1");
});

sk_test!(opt_nth_range_dec, "f1,f2,f3,f4", &["--delimiter", ",", "--nth", "3..2"], {
  @capture[1] contains("1/1");
  @capture[2] eq("> f1,f2,f3,f4");

  @keys Str("1");
  @capture[0] eq("> 1");
  @capture[1] contains("0/1");
});

sk_test!(opt_print_query, "10\\n20\\n30", &["-q", "2", "--print-query"], {
  @capture[2] eq("> 20");
  @keys Enter;
  @capture[0] ne("> 2");

  @dbg;
  @output[0] eq("2");
  @output[1] eq("20");
});

sk_test!(opt_print_cmd, "1\\n2\\n3", &["--cmd-query", "cmd", "--print-cmd"], {
  @lines |l| (l.len() > 4);
  @capture[0] starts_with(">");
  @capture[2] eq("> 1");
  @keys Enter;
  @output[0] eq("cmd");
  @output[1] eq("1");
});

sk_test!(opt_print_cmd_and_query, "10\\n20\\n30", &["--cmd-query", "cmd", "--print-cmd", "-q", "2", "--print-query"], {
  @capture[0] starts_with("> 2");
  @capture[2] eq("> 20");
  @keys Enter;
  @output[0] eq("2");
  @output[1] eq("cmd");
  @output[2] eq("20");
});

sk_test!(opt_hscroll_begin, &format!("b{}", &["a"; 1000].join("")), &["-q", "b"], {
  @capture[2] ends_with("..");
});

sk_test!(opt_hscroll_middle, &format!("{}b{}", &["a"; 1000].join(""), &["a"; 1000].join("")), &["-q", "b"], {
  @capture[2] ends_with("..");
  @capture[2] starts_with("> ..");
});

sk_test!(opt_hscroll_end, &format!("{}b", &["a"; 1000].join("")), &["-q", "b"], {
  @capture[2] starts_with("> ..");
});

sk_test!(opt_no_hscroll, &format!("{}b", &["a"; 1000].join("")), &["-q", "b", "--no-hscroll"], {
  @lines |l| (l.len() > 2 && !l[2].starts_with("> .."));
  @capture[2] ends_with("..");
});

sk_test!(opt_tabstop_default, "a\\tb", &[], {
  @capture[2] starts_with("> a       b");
});

sk_test!(opt_tabstop_1, "a\\tb", &["--tabstop", "1"], {
  @capture[2] starts_with("> a b");
});

sk_test!(opt_tabstop_3, "aa\\tb", &["--tabstop", "3"], {
  @capture[2] starts_with("> aa b");
});

sk_test!(opt_info_control, "a\\nb\\nc", &[], {
  @capture[0] starts_with(">");
  @capture[1] starts_with("  3/3");
  @capture[1] ends_with("0/0");

  @keys Key('a');
  @capture[1] starts_with("  1/3");
  @capture[1] ends_with("0/0");
});

sk_test!(opt_info_default, "a\\nb\\nc", &["--info", "default"], {
  @capture[0] starts_with(">");
  @capture[1] starts_with("  3/3");
  @capture[1] ends_with("0/0");

  @keys Key('a');
  @capture[1] starts_with("  1/3");
  @capture[1] ends_with("0/0");
});

sk_test!(opt_no_info, "a\\nb\\nc", &["--no-info"], {
  @capture[0] eq(">");
  @capture[1] eq("> a");
});

sk_test!(opt_info_hidden, "a\\nb\\nc", &["--info", "hidden"], {
  @capture[0] eq(">");
  @capture[1] eq("> a");
});

sk_test!(opt_info_inline, "a\\nb\\nc", &["--info", "inline"], {
  @lines |l| (l.len() > 0 && l[0].starts_with(">   < 3/3") && l[0].ends_with("0/0"));
  @capture[0] starts_with(">   < 3/3");
  @capture[0] ends_with("0/0");

  @keys Key('a');
  @capture[0] starts_with("> a  < 1/3");
  @capture[0] ends_with("0/0");
});

sk_test!(opt_inline_info, "a\\nb\\nc", &["--inline-info"], {
  @capture[0] starts_with(">   < 3/3");
  @capture[0] ends_with("0/0");

  @keys Key('a');
  @capture[0] starts_with("> a  < 1/3");
  @capture[0] ends_with("0/0");
});

sk_test!(opt_header_only, "a\\nb\\nc", &["--header", "test_header"], {
  @capture[2] trim().eq("test_header");
});

sk_test!(opt_header_inline_info, "a\\nb\\nc", &["--header", "test_header", "--inline-info"], {
  @capture[1] trim().eq("test_header");
});

sk_test!(opt_header_reverse, @cmd "echo -e -n 'a\\nb\\nc'", &["--header", "test_header", "--reverse"], {
  @capture[-1] starts_with(">");
  @capture[-3] trim().eq("test_header");
});

sk_test!(opt_header_reverse_inline_info, @cmd "echo -e -n 'a\\nb\\nc'", &["--header", "test_header", "--reverse", "--inline-info"], {
  @capture[-1] starts_with(">");
  @capture[-2] trim().eq("test_header");
});

sk_test!(opt_header_lines_1, "a\\nb\\nc", &["--header-lines", "1"], {
  @capture[2] trim().eq("a");
  @capture[3] starts_with(">");
});

sk_test!(opt_header_lines_all, "a\\nb\\nc", &["--header-lines", "4"], {
  @capture[2] trim().eq("a");
  @capture[3] trim().eq("b");
  @capture[4] trim().eq("c");
});

sk_test!(opt_header_lines_inline_info, "a\\nb\\nc", &["--header-lines", "1", "--inline-info"], {
  @capture[1] trim().eq("a");
});

sk_test!(opt_header_lines_reverse, @cmd "echo -e -n 'a\\nb\\nc'", &["--header-lines", "1", "--reverse"], {
  @capture[-1] starts_with(">");
  @capture[-3] trim().eq("a");
  @capture[-4] trim().eq("> b");
});

sk_test!(opt_header_lines_reverse_inline_info, @cmd "echo -e -n 'a\\nb\\nc'", &["--header-lines", "1", "--reverse", "--inline-info"], {
  @capture[-1] starts_with(">");
  @capture[-2] trim().eq("a");
  @capture[-3] trim().eq("> b");
});

sk_test!(opt_reserved_options, "a\\nb", &[], tmux => {
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
});

sk_test!(opt_multiple_flags_basic, "a\\nb", &[], tmux => {
  let basic_flags = [
      "--bind=ctrl-a:cancel --bind ctrl-b:cancel",
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
});

sk_test!(opt_multiple_flags_prompt, "", &["--prompt a", "--prompt b", "-p c"], {
  @capture[0] starts_with("c");
});

sk_test!(opt_multiple_flags_cmd_prompt, "", &["-i", "--cmd-prompt a", "--cmd-prompt c"], {
  @capture[0] starts_with("c");
});

sk_test!(opt_multiple_flags_cmd_query, "", &["-i", "--cmd-query a", "--cmd-query b"], {
  @capture[0] starts_with("c> b");
});

sk_test!(opt_multiple_flags_interactive, "", &["-i", "--interactive", "--interactive"], {
  @capture[0] starts_with("c>");
});

sk_test!(opt_multiple_flags_reverse, "", &["--reverse", "--reverse"], {
  @capture[-1] starts_with(">");
});

sk_test!(opt_multiple_flags_combined_nth, "a b c\\nd e f", &["--nth 1,2"], {
  @keys Key('c');
  @capture[1] contains("0/2");
});

sk_test!(opt_multiple_flags_combined_with_nth, "a b c\\nd e f", &["--with-nth 1,2"], {
  @capture[2] ends_with("a b");
  @capture[3] ends_with("d e");
});

sk_test!(opt_ansi_null, "a\\0b", &["--ansi"], {
  @capture[1] trim().starts_with("1/1");
  @keys Enter;
  @output[0] contains("\0");
});

sk_test!(opt_skip_to_pattern, "a/b/c", &["--skip-to-pattern", "'[^/]*$'", "--bind", "ctrl-a:scroll-left", "--bind", "ctrl-x:scroll-right"], {
  @capture[2] starts_with("> ..c");
  @keys Ctrl(&Key('a'));
  @capture[2] starts_with("> ../c");
  @keys Ctrl(&Key('x'));
  @capture[2] starts_with("> ..c");
});

sk_test!(opt_multi, "a\\nb\\nc", &["--multi"], {
  @capture[4] trim().eq("c");

  @keys BTab;
  @capture[2] trim().eq(">a");
  @capture[3] trim().eq("> b");
  @keys BTab;
  @capture[2] trim().eq(">a");
  @capture[3] trim().eq(">b");
  @capture[4] trim().eq("> c");
  @keys Enter;

  @output[0] trim().eq("a");
  @output[1] trim().eq("b");
});

sk_test!(opt_pre_select_n, "a\\nb\\nc", &["-m", "--pre-select-n", "2"], {
  @capture[2] eq(">>a");
  @capture[3] trim().eq(">b");
});

sk_test!(opt_pre_select_items, "a\\nb\\nc", &["-m", "--pre-select-items", "$'b\\nc'"], {
  @capture[2] trim().eq("> a");
  @capture[3] trim().eq(">b");
  @capture[4] trim().eq(">c");
});

sk_test!(opt_pre_select_pat, "a\\nb\\nc", &["-m", "--pre-select-pat", "'[b|c]'"], {
  @capture[2] trim().eq("> a");
  @capture[3] trim().eq(">b");
  @capture[4] trim().eq(">c");
});

sk_test!(opt_pre_select_file, "a\\nb\\nc", &[], tmux => {
  let mut pre_select_file = NamedTempFile::new()?;
  pre_select_file.write(b"b\nc")?;
  let tmux = setup(
      "a\\nb\\nc",
      &["-m", "--pre-select-file", pre_select_file.path().to_str().unwrap()],
  )?;
  tmux.until(|l| l.len() > 4 && l[2] == "> a" && l[3].trim() == ">b" && l[4].trim() == ">c")?;
});

sk_test!(opt_no_clear_if_empty, @cmd "echo -ne 'a\\nb\\nc'", &["-i", "--no-clear-if-empty", "-c", "'echo -ne {}'"], {
  @capture[0] trim().eq("c>");

  @keys Str("xxxx");
  @capture[0] trim().eq("c> xxxx");
  @capture[1] trim().starts_with("1/1");

  @keys Ctrl(&Key('w'));
  @capture[0] trim().starts_with("c>");
  @capture[1] trim().starts_with("0/0");
  @capture[2] trim().starts_with("> xxxx");
});

sk_test!(opt_accept_arg, "a\\nb", &["--bind", "ctrl-a:accept:hello"], {
  @capture[1] trim().starts_with("2/2");
  @keys Ctrl(&Key('a'));
  @output[0] eq("hello");
  @output[1] eq("a");
});

sk_test!(opt_tac, "a\\nb", &["--tac"], {
  @capture[1] trim().starts_with("2/2");
  @capture[2] starts_with("> b");
  @capture[3] contains("a");
});

sk_test!(opt_tac_with_header_lines, "a\\nb\\nc\\nd\\ne", &["--tac", "--header-lines", "2"], {
  // Should have 3 selectable items (c, d, e reversed to e, d, c)
  // The count shows matched/total: 3 matched out of 3 selectable (5 total items with 2 headers)
  @capture[1] trim().starts_with("5/3");

  // Headers should be first 2 items from input (a, b) in original order
  @capture[2] trim().eq("a");
  @capture[3] trim().eq("b");

  // First selectable item should be 'e' (last from input, first in reversed order)
  @capture[4] starts_with("> e");
});

sk_test!(opt_replstr, "", &["-I", "..", "-i", "-c", "'echo foo {} ..'"], {
    @capture[0] starts_with("c>");
    @capture[2] starts_with("> foo {}");
    @keys Key('a');
    @capture[2] starts_with("> foo {} a");
});

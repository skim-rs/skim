#[allow(dead_code)]
mod common;

use common::Keys::*;
use common::TmuxController;

#[test]
fn test_highlight_match() {
    let tmux = TmuxController::new().unwrap();
    let outfile = tmux
        .start_sk(
            Some("echo -e 'apple\\nbanana\\ngrape'"),
            &["--color=matched:9,current_match:1"],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("apple")))
        .unwrap();

    // Type 'pp' to filter items - this should only match "apple"
    tmux.send_keys(&[Key('p'), Key('p')]).unwrap();

    // Wait for filtering to complete - should only show apple
    tmux.until(|lines| {
        let has_apple = lines.iter().any(|line| line.contains("apple"));
        let has_banana = lines.iter().any(|line| line.contains("banana"));
        let has_grape = lines.iter().any(|line| line.contains("grape"));
        has_apple && !has_banana && !has_grape
    })
    .unwrap();

    // Capture the colored output
    let colored_lines = tmux.capture_colored().unwrap();

    // Find the line containing apple text (may be split across spans due to highlighting)
    let apple_line = colored_lines
        .iter()
        .find(|line| line.contains("a") && line.contains("pp") && line.contains("le"))
        .expect("Should find a line containing apple text");

    // Check that the 'p' characters in "apple" have highlighting color codes
    // Based on debug output, we should see Color::Indexed(1) which is \x1b[38;5;1m
    // Look for the foreground color code for the matched characters
    let has_match_highlight = apple_line.contains("\x1b[38;5;1m") || apple_line.contains("\x1b[38;5;9m");

    assert!(
        has_match_highlight,
        "Apple line should contain highlighting color codes (38;5;1 or 38;5;9). Line: {}",
        apple_line.replace('\x1b', "\\e")
    );

    // Also verify that the highlighting is specifically applied to the 'pp' characters
    assert!(
        apple_line.contains("pp\x1b[39m") || apple_line.contains("pp\x1b["),
        "The 'pp' characters should be highlighted. Line: {}",
        apple_line.replace('\x1b', "\\e")
    );

    tmux.send_keys(&[Enter]).unwrap();
    let output = tmux.output(&outfile).unwrap();
    assert_eq!(output, &["apple"]);
}

#[test]
fn test_highlight_split_match() {
    let tmux = TmuxController::new().unwrap();
    let outfile = tmux
        .start_sk(
            Some("echo -e 'apple\\nbanana\\ngrape'"),
            &["--color=matched:9,current_match:1"],
        )
        .unwrap();

    tmux.until(|lines| lines.iter().any(|line| line.contains("apple")))
        .unwrap();

    // Type 'aaa' to filter items - this should only match "banana" (which has 3 'a's)
    tmux.send_keys(&[Key('a'), Key('a'), Key('a')]).unwrap();

    // Wait for filtering to complete - should only show banana
    tmux.until(|lines| {
        let has_apple = lines.iter().any(|line| line.contains("apple"));
        let has_banana = lines.iter().any(|line| line.contains("banana"));
        let has_grape = lines.iter().any(|line| line.contains("grape"));
        !has_apple && has_banana && !has_grape
    })
    .unwrap();

    // Capture the colored output
    let colored_lines = tmux.capture_colored().unwrap();

    // Find the line containing banana text (may be split across spans due to highlighting)
    let banana_line = colored_lines
        .iter()
        .find(|line| line.contains("b") && line.contains("a") && line.contains("n"))
        .expect("Should find a line containing banana text");

    // Check that the 'a' characters in "banana" have highlighting color codes
    // We expect to see Color::Indexed(1) which is \x1b[38;5;1m or Color::Indexed(9) which is \x1b[38;5;9m
    let has_match_highlight = banana_line.contains("\x1b[38;5;1m") || banana_line.contains("\x1b[38;5;9m");

    assert!(
        has_match_highlight,
        "Banana line should contain highlighting color codes (38;5;1 or 38;5;9). Line: {}",
        banana_line.replace('\x1b', "\\e")
    );

    // Verify that we have exactly 3 individual 'a' character highlights in "banana"
    // From the debug output, we can see each 'a' is highlighted individually:
    // b\e[38;5;1m\e[48;5;236ma\e[39m\e[49mn\e[38;5;1m\e[48;5;236ma\e[39m\e[49mn\e[38;5;1m\e[48;5;236ma

    // Count individual 'a' highlights by looking for the pattern:
    // \e[38;5;1m\e[48;5;236ma followed by reset codes or another highlight
    let highlight_pattern = "\x1b[38;5;1m\x1b[48;5;236ma";
    let individual_a_highlights = banana_line.matches(highlight_pattern).count();

    assert_eq!(
        individual_a_highlights,
        3,
        "Should have exactly 3 individual 'a' character highlights in banana. Found {} individual highlights. Full line: {}",
        individual_a_highlights,
        banana_line.replace('\x1b', "\\e")
    );

    tmux.send_keys(&[Enter]).unwrap();
    let output = tmux.output(&outfile).unwrap();
    assert_eq!(output, &["banana"]);
}


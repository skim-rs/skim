use super::*;

#[test]
fn test_preview_layout_direction_only() {
    let layout = PreviewLayout::from("left");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.size, Size::Percent(50)); // default
    assert!(!layout.hidden);
    assert_eq!(layout.offset, None);

    let layout = PreviewLayout::from("right");
    assert_eq!(layout.direction, Direction::Right);

    let layout = PreviewLayout::from("up");
    assert_eq!(layout.direction, Direction::Up);

    let layout = PreviewLayout::from("down");
    assert_eq!(layout.direction, Direction::Down);
}

#[test]
fn test_preview_layout_with_size() {
    let layout = PreviewLayout::from("left:30%");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.size, Size::Percent(30));
    assert!(!layout.hidden);
    assert_eq!(layout.offset, None);

    let layout = PreviewLayout::from("right:40");
    assert_eq!(layout.direction, Direction::Right);
    assert_eq!(layout.size, Size::Fixed(40));
}

#[test]
fn test_preview_layout_with_offset() {
    let layout = PreviewLayout::from("left:+123");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.offset, Some("+123".to_string()));

    let layout = PreviewLayout::from("left:+{2}");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.offset, Some("+{2}".to_string()));

    let layout = PreviewLayout::from("left:+{2}-2");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.offset, Some("+{2}-2".to_string()));
}

#[test]
fn test_preview_layout_with_size_and_offset() {
    let layout = PreviewLayout::from("left:50%:+{2}");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.size, Size::Percent(50));
    assert_eq!(layout.offset, Some("+{2}".to_string()));

    let layout = PreviewLayout::from("right:40:+123");
    assert_eq!(layout.direction, Direction::Right);
    assert_eq!(layout.size, Size::Fixed(40));
    assert_eq!(layout.offset, Some("+123".to_string()));
}

#[test]
fn test_preview_layout_with_hidden() {
    let layout = PreviewLayout::from("left:hidden");
    assert_eq!(layout.direction, Direction::Left);
    assert!(layout.hidden);

    let layout = PreviewLayout::from("right:50%:hidden");
    assert_eq!(layout.direction, Direction::Right);
    assert_eq!(layout.size, Size::Percent(50));
    assert!(layout.hidden);
}

#[test]
fn test_preview_layout_complex() {
    let layout = PreviewLayout::from("left:30%:+{2}-5:hidden");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.size, Size::Percent(30));
    assert_eq!(layout.offset, Some("+{2}-5".to_string()));
    assert!(layout.hidden);
}

#[test]
fn test_preview_layout_toggle_negations() {
    // The explicit `no*` spellings clear each boolean flag.
    let layout = PreviewLayout::from("left:nohidden:nowrap:nopty");
    assert!(!layout.hidden);
    assert!(!layout.wrap);
    assert!(!layout.pty);
}

#[test]
fn test_preview_layout_wrap_and_pty_enabled() {
    let layout = PreviewLayout::from("up:wrap:pty");
    assert_eq!(layout.direction, Direction::Up);
    assert!(layout.wrap);
    assert!(layout.pty);
}

#[test]
fn test_preview_layout_skips_empty_parts() {
    // Consecutive colons yield empty parts that are skipped.
    let layout = PreviewLayout::from("left::50%");
    assert_eq!(layout.direction, Direction::Left);
    assert_eq!(layout.size, Size::Percent(50));
}

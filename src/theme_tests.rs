use super::*;

#[test]
fn test_base_themes() {
    // Test that base themes have expected properties
    let none = ColorTheme::none();
    // Spinner should be bold even in none theme
    assert!(none.spinner.add_modifier.contains(Modifier::BOLD));

    let bw = ColorTheme::bw();
    assert!(bw.matched.add_modifier.contains(Modifier::UNDERLINED));
    assert!(bw.current.add_modifier.contains(Modifier::REVERSED));

    let theme_16 = ColorTheme::default16();
    assert_eq!(theme_16.matched.fg, Some(Color::Green));
    assert_eq!(theme_16.matched.bg, None);

    let dark = ColorTheme::dark256();
    assert_eq!(dark.matched.fg, Some(Color::Indexed(108)));
    assert_eq!(dark.matched.bg, Some(Color::Indexed(0)));

    let molokai = ColorTheme::molokai256();
    assert_eq!(molokai.matched.fg, Some(Color::Indexed(234)));
    assert_eq!(molokai.matched.bg, Some(Color::Indexed(186)));

    let light = ColorTheme::light256();
    assert_eq!(light.matched.fg, Some(Color::Indexed(0)));
    assert_eq!(light.matched.bg, Some(Color::Indexed(220)));
}

#[test]
fn test_from_options_base_themes() {
    // Test base theme names
    let dark = ColorTheme::from_options("dark");
    assert!(dark.matched.fg.is_some());

    let molokai = ColorTheme::from_options("molokai");
    assert!(molokai.matched.fg.is_some());

    let light = ColorTheme::from_options("light");
    assert!(light.matched.fg.is_some());

    let theme_16 = ColorTheme::from_options("16");
    assert!(theme_16.matched.fg.is_some());

    let bw = ColorTheme::from_options("bw");
    assert!(bw.matched.add_modifier.contains(Modifier::UNDERLINED));

    // Test that "none" theme uses reset style (which may have default colors from terminal)
    let none = ColorTheme::from_options("none");
    // Spinner should still be bold even in none theme
    assert!(none.spinner.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_ansi_color_parsing() {
    // Test ANSI color (0-255)
    let theme = ColorTheme::from_options("matched:108");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));

    let theme = ColorTheme::from_options("prompt:25");
    assert_eq!(theme.prompt.fg, Some(Color::Indexed(25)));
}

#[test]
fn test_rgb_hex_color_parsing() {
    // Test RGB hex color (#rrggbb)
    let theme = ColorTheme::from_options("matched:#ff0000");
    assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 0, 0)));

    let theme = ColorTheme::from_options("prompt:#00ff00");
    assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 255, 0)));

    let theme = ColorTheme::from_options("info:#0000ff");
    assert_eq!(theme.info.fg, Some(Color::Rgb(0, 0, 255)));
}

#[test]
fn test_color_with_modifiers() {
    // Test color with bold modifier
    let theme = ColorTheme::from_options("matched:108:bold");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));

    // Test color with underline modifier
    let theme = ColorTheme::from_options("matched:108:underlined");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));

    // Test color with multiple modifiers (using +)
    let theme = ColorTheme::from_options("matched:108:bold:underlined");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
    assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn test_modifier_shortcuts() {
    // Test short modifier names
    let theme = ColorTheme::from_options("matched:108:b");
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));

    let theme = ColorTheme::from_options("matched:108:u");
    assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));

    let theme = ColorTheme::from_options("matched:108:i");
    assert!(theme.matched.add_modifier.contains(Modifier::ITALIC));

    let theme = ColorTheme::from_options("matched:108:r");
    assert!(theme.matched.add_modifier.contains(Modifier::REVERSED));

    let theme = ColorTheme::from_options("matched:108:d");
    assert!(theme.matched.add_modifier.contains(Modifier::DIM));

    let theme = ColorTheme::from_options("matched:108:c");
    assert!(theme.matched.add_modifier.contains(Modifier::CROSSED_OUT));
}

#[test]
fn test_regular_modifier_reset() {
    // Test that 'regular' or 'x' resets modifiers
    let theme = ColorTheme::from_options("matched:108:x:bold");
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
    assert!(!theme.matched.add_modifier.contains(Modifier::ITALIC));

    let theme = ColorTheme::from_options("matched:108:regular:underlined");
    assert!(theme.matched.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn test_multiple_color_components() {
    // Test multiple color components separated by comma
    let theme = ColorTheme::from_options("matched:108,prompt:25");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    assert_eq!(theme.prompt.fg, Some(Color::Indexed(25)));

    let theme = ColorTheme::from_options("matched:#ff0000:bold,prompt:#00ff00:underlined");
    assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 0, 0)));
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
    assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 255, 0)));
    assert!(theme.prompt.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn test_component_name_aliases() {
    // Test that aliases work correctly
    let theme = ColorTheme::from_options("hl:108");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));

    let theme = ColorTheme::from_options("fg+:254");
    assert_eq!(theme.current.fg, Some(Color::Indexed(254)));

    let theme = ColorTheme::from_options("bg+:236");
    assert_eq!(theme.current.bg, Some(Color::Indexed(236)));

    let theme = ColorTheme::from_options("hl+:151");
    // hl+ is an alias for current_match
    assert_eq!(theme.current_match.fg, Some(Color::Indexed(151)));

    let theme = ColorTheme::from_options("pointer:161");
    assert_eq!(theme.cursor.fg, Some(Color::Indexed(161)));

    let theme = ColorTheme::from_options("marker:168");
    assert_eq!(theme.selected.fg, Some(Color::Indexed(168)));
}

#[test]
fn test_background_color() {
    // Test setting background color explicitly
    let theme = ColorTheme::from_options("matched_bg:0");
    assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));

    let theme = ColorTheme::from_options("matched-bg:236");
    assert_eq!(theme.matched.bg, Some(Color::Indexed(236)));
}

#[test]
fn test_default_theme_with_overrides() {
    // Test overriding default theme
    let theme = ColorTheme::from_options("default,matched:200");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(200)));
    // Other colors should still be from default theme
    assert!(theme.prompt.fg.is_some());
}

#[test]
fn test_theme_with_overrides() {
    // Test overriding theme
    for opts in &["16,prompt:200", "prompt:150,16,prompt:200"] {
        let theme = ColorTheme::from_options(opts);
        assert_eq!(theme.prompt.fg, Some(Color::Indexed(200)));
        // Other colors should still be from given theme
        assert_eq!(theme.matched.fg, Some(Color::Green));
        assert_eq!(theme.matched.bg, None);
    }
}

#[test]
fn test_all_component_names() {
    // Test all valid component names with their specific colors
    let theme = ColorTheme::from_options("normal:108");
    assert_eq!(theme.normal.fg, Some(Color::Indexed(108)));

    let theme = ColorTheme::from_options("matched:109");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(109)));

    let theme = ColorTheme::from_options("current:110");
    assert_eq!(theme.current.fg, Some(Color::Indexed(110)));

    let theme = ColorTheme::from_options("current_match:111");
    // current_match should now correctly set current_match.fg
    assert_eq!(theme.current_match.fg, Some(Color::Indexed(111)));

    let theme = ColorTheme::from_options("query:112");
    assert_eq!(theme.query.fg, Some(Color::Indexed(112)));

    let theme = ColorTheme::from_options("spinner:113");
    assert_eq!(theme.spinner.fg, Some(Color::Indexed(113)));

    let theme = ColorTheme::from_options("info:114");
    assert_eq!(theme.info.fg, Some(Color::Indexed(114)));

    let theme = ColorTheme::from_options("prompt:115");
    assert_eq!(theme.prompt.fg, Some(Color::Indexed(115)));

    let theme = ColorTheme::from_options("cursor:116");
    assert_eq!(theme.cursor.fg, Some(Color::Indexed(116)));

    let theme = ColorTheme::from_options("selected:117");
    assert_eq!(theme.selected.fg, Some(Color::Indexed(117)));

    let theme = ColorTheme::from_options("header:118");
    assert_eq!(theme.header.fg, Some(Color::Indexed(118)));

    let theme = ColorTheme::from_options("border:119");
    assert_eq!(theme.border.fg, Some(Color::Indexed(119)));
}

#[test]
fn test_invalid_color_graceful_handling() {
    // Test that invalid color values don't crash
    // When color is invalid (not a number), it returns None and the color isn't set
    // So the theme starts with dark256() and the invalid color spec doesn't change it
    let theme = ColorTheme::from_options("matched:invalid");
    // Should remain the dark256 default since invalid color is ignored
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
    assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));

    // Invalid hex digits in #rrggbb format will use unwrap_or(255) fallback
    // So "#gggggg" becomes Rgb(255, 255, 255) since 'gg' is invalid hex
    let theme = ColorTheme::from_options("matched:#gggggg");
    assert_eq!(theme.matched.fg, Some(Color::Rgb(255, 255, 255)));
    // But the background remains from dark256 theme since we only set fg
    assert_eq!(theme.matched.bg, Some(Color::Indexed(0)));
}

#[test]
fn test_init_from_options() {
    // Test initialization from SkimOptions
    let opts = crate::options::SkimOptionsBuilder::default()
        .color("matched:108")
        .build()
        .unwrap();
    let theme = ColorTheme::init_from_options(&opts);
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
}

#[test]
fn test_complex_color_spec() {
    // Test a complex real-world color specification
    let theme = ColorTheme::from_options("dark,matched:#00ff00:bold,prompt:#0000ff:underlined,current:#ffff00:italic");
    assert_eq!(theme.matched.fg, Some(Color::Rgb(0, 255, 0)));
    assert!(theme.matched.add_modifier.contains(Modifier::BOLD));
    assert_eq!(theme.prompt.fg, Some(Color::Rgb(0, 0, 255)));
    assert!(theme.prompt.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(theme.current.fg, Some(Color::Rgb(255, 255, 0)));
    assert!(theme.current.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_minus_one_color_reset() {
    // `hl:-1:reverse` should not have foreground or background color, but keep the reverse modifier
    let theme = ColorTheme::from_options("dark,hl:-1:reverse,hl-bg:-1,hl+:-1:bold,bg+:-1");
    assert_eq!(theme.matched.fg, Some(Color::Reset));
    assert_eq!(theme.matched.bg, Some(Color::Reset));
    assert!(theme.matched.add_modifier.contains(Modifier::REVERSED));
    assert_eq!(theme.current_match.fg, Some(Color::Reset));
    assert_ne!(theme.current_match.bg, Some(Color::Reset));
    assert!(theme.current_match.add_modifier.contains(Modifier::BOLD));
    let theme = ColorTheme::from_options("dark,prompt:-1:underlined");
    assert_eq!(theme.prompt.fg, Some(Color::Reset));
    assert!(theme.prompt.add_modifier.contains(Modifier::UNDERLINED));
    let theme = ColorTheme::from_options("dark,bg+:-1");
    assert_eq!(theme.current.bg, Some(Color::Reset));
}

#[test]
fn test_catppuccin_themes_have_colors() {
    for theme in [
        ColorTheme::catppuccin_mocha(),
        ColorTheme::catppuccin_macchiato(),
        ColorTheme::catppuccin_latte(),
        ColorTheme::catppuccin_frappe(),
    ] {
        assert!(theme.matched.fg.is_some());
        assert!(theme.current.bg.is_some());
    }
}

#[test]
fn test_from_options_catppuccin_aliases() {
    // Both underscore and hyphen spellings must resolve to a populated theme.
    for name in [
        "catppuccin_mocha",
        "catppuccin-mocha",
        "catppuccin_macchiato",
        "catppuccin-macchiato",
        "catppuccin_latte",
        "catppuccin-latte",
        "catppuccin_frappe",
        "catppuccin-frappe",
    ] {
        let theme = ColorTheme::from_options(name);
        assert!(theme.matched.fg.is_some(), "theme {name} should have a matched fg");
    }
}

#[test]
fn test_from_options_default_and_empty_aliases() {
    let default = ColorTheme::from_options("default");
    assert_eq!(default.matched.fg, ColorTheme::dark256().matched.fg);

    let empty = ColorTheme::from_options("empty");
    assert!(empty.spinner.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_from_options_unknown_falls_back_to_dark() {
    let unknown = ColorTheme::from_options("this-is-not-a-real-theme");
    assert_eq!(unknown.matched.fg, ColorTheme::dark256().matched.fg);
}

#[test]
fn test_unknown_modifier_is_ignored() {
    // An unrecognised modifier name is dropped without affecting the color.
    let theme = ColorTheme::from_options("matched:108:notamodifier");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(108)));
}

#[test]
fn test_explicit_fg_layer_suffix() {
    let theme = ColorTheme::from_options("matched_fg:5");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(5)));
    let theme = ColorTheme::from_options("matched-fg:6");
    assert_eq!(theme.matched.fg, Some(Color::Indexed(6)));
}

#[test]
fn test_underline_layer_suffixes() {
    // `_u` / `-u` and the long `_underline` form set the underline color.
    let theme = ColorTheme::from_options("matched_u:5");
    assert_eq!(theme.matched.underline_color, Some(Color::Indexed(5)));
    let theme = ColorTheme::from_options("matched-u:6");
    assert_eq!(theme.matched.underline_color, Some(Color::Indexed(6)));
    let theme = ColorTheme::from_options("matched_underline:7");
    assert_eq!(theme.matched.underline_color, Some(Color::Indexed(7)));
    let theme = ColorTheme::from_options("matched-underline:8");
    assert_eq!(theme.matched.underline_color, Some(Color::Indexed(8)));
}

#[test]
fn test_bare_bg_sets_normal_background() {
    // A bare "bg" name targets the normal style's background.
    let theme = ColorTheme::from_options("bg:5");
    assert_eq!(theme.normal.bg, Some(Color::Indexed(5)));
}

#[test]
fn test_unknown_component_name_is_noop() {
    // An unknown component returns early, leaving the dark256 default intact.
    let theme = ColorTheme::from_options("not_a_component:5");
    assert_eq!(theme.matched.fg, ColorTheme::dark256().matched.fg);
}

struct EnvGuard {
    key: &'static str,
    prior: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var_os(key);
        // SAFETY: caller must hold a serial lock so no other thread reads this var.
        unsafe { std::env::set_var(key, value) };
        Self { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: same serial-lock guarantee as set().
        unsafe {
            match &self.prior {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
#[serial_test::serial]
fn test_init_from_options_respects_no_color() {
    let _guard = EnvGuard::set("NO_COLOR", "1");
    let opts = crate::options::SkimOptionsBuilder::default().build().unwrap();
    let theme = ColorTheme::init_from_options(&opts);
    // NO_COLOR yields the `none` theme, matching the bare none() palette.
    assert_eq!(theme.matched.fg, ColorTheme::none().matched.fg);
}

#[test]
#[serial_test::serial]
fn test_init_from_options_empty_no_color_uses_default() {
    let _guard = EnvGuard::set("NO_COLOR", "");
    let opts = crate::options::SkimOptionsBuilder::default().build().unwrap();
    let theme = ColorTheme::init_from_options(&opts);
    // An empty NO_COLOR is ignored, so the dark256 default applies.
    assert_eq!(theme.matched.fg, ColorTheme::dark256().matched.fg);
}

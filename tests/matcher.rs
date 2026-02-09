#[allow(dead_code)]
#[macro_use]
mod common;

const INPUT_ITEMS: [&str; 49] = [
    "src/util.rs",
    "src/completions.rs",
    "src/manpage.rs",
    "src/tmux.rs",
    "src/helper/mod.rs",
    "src/helper/selector.rs",
    "src/helper/item_reader.rs",
    "src/helper/item.rs",
    "src/helper/macros.rs",
    "src/options.rs",
    "src/field.rs",
    "src/matcher.rs",
    "src/output.rs",
    "src/item.rs",
    "src/bin/main.rs",
    "src/binds.rs",
    "src/prelude.rs",
    "src/theme.rs",
    "src/engine/mod.rs",
    "src/engine/util.rs",
    "src/engine/fuzzy.rs",
    "src/engine/all.rs",
    "src/engine/split.rs",
    "src/engine/regexp.rs",
    "src/engine/factory.rs",
    "src/engine/normalized.rs",
    "src/engine/exact.rs",
    "src/engine/andor.rs",
    "src/skim_item.rs",
    "src/fuzzy_matcher/mod.rs",
    "src/fuzzy_matcher/util.rs",
    "src/fuzzy_matcher/skim.rs",
    "src/fuzzy_matcher/frizbee.rs",
    "src/fuzzy_matcher/clangd.rs",
    "src/tui/mod.rs",
    "src/tui/util.rs",
    "src/tui/options.rs",
    "src/tui/widget.rs",
    "src/tui/preview.rs",
    "src/tui/item_list.rs",
    "src/tui/event.rs",
    "src/tui/backend.rs",
    "src/tui/app.rs",
    "src/tui/input.rs",
    "src/tui/statusline.rs",
    "src/tui/header.rs",
    "src/lib.rs",
    "src/spinlock.rs",
    "src/reader.rs",
];

insta_test!(matcher_default, INPUT_ITEMS, &["-q", "stum"], {
    @snap;
});

insta_test!(matcher_skim_v2, INPUT_ITEMS, &["-q", "stum", "--algo", "skim_v2"], {
    @snap;
});
insta_test!(matcher_skim_v1, INPUT_ITEMS, &["-q", "stum", "--algo", "skim_v1"], {
    @snap;
});
insta_test!(matcher_clangd, INPUT_ITEMS, &["-q", "stum", "--algo", "clangd"], {
    @snap;
});
insta_test!(matcher_frizbee, INPUT_ITEMS, &["-q", "stum", "--algo", "frizbee"], {
    @snap;
});

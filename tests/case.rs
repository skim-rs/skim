#[allow(dead_code)]
#[macro_use]
mod common;

// Smart case: lowercase query matches case-insensitively
insta_test!(case_smart_lower, ["aBcDeF"], &["--case", "smart"], {
    @snap;
    @type "abc";
    @snap;
});

// Smart case: mixed-case query matches case-sensitively
insta_test!(case_smart_exact, ["aBcDeF"], &["--case", "smart"], {
    @snap;
    @type "aBc";
    @snap;
});

// Smart case: uppercase query doesn't match different case
insta_test!(case_smart_no_match, ["aBcDeF"], &["--case", "smart"], {
    @snap;
    @type "Abc";
    @snap;
});

// Ignore case: lowercase query matches
insta_test!(case_ignore_lower, ["aBcDeF"], &["--case", "ignore"], {
    @snap;
    @type "abc";
    @snap;
});

// Ignore case: exact case matches
insta_test!(case_ignore_exact, ["aBcDeF"], &["--case", "ignore"], {
    @snap;
    @type "aBc";
    @snap;
});

// Ignore case: different case matches
insta_test!(case_ignore_different, ["aBcDeF"], &["--case", "ignore"], {
    @snap;
    @type "Abc";
    @snap;
});

// Ignore case: non-matching character doesn't match
insta_test!(case_ignore_no_match, ["aBcDeF"], &["--case", "ignore"], {
    @snap;
    @type "z";
    @snap;
});

// Respect case: lowercase query doesn't match different case
insta_test!(case_respect_lower, ["aBcDeF"], &["--case", "respect"], {
    @snap;
    @type "abc";
    @snap;
});

// Respect case: exact case matches
insta_test!(case_respect_exact, ["aBcDeF"], &["--case", "respect"], {
    @snap;
    @type "aBc";
    @snap;
});

// Respect case: different case doesn't match
insta_test!(case_respect_no_match, ["aBcDeF"], &["--case", "respect"], {
    @snap;
    @type "Abc";
    @snap;
});

// Non-ascii input

insta_test!(case_non_ascii, ["слово", "Слово", "СЛОВО"], &["--case", "smart"], {
    @snap;
    @type "слово";
    @snap;
    @ctrl 'w';
    @type "Слово";
    @snap;
});

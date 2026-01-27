#[allow(dead_code)]
#[macro_use]
mod common;

// Test normalize: accented item matches unaccented query
insta_test!(insta_normalize_accented_item_unaccented_query, ["café", "cafe", "tea"], &["--normalize"], {
    @snap;
    @type "cafe";
    @snap;
});

// Test normalize: unaccented item matches accented query
insta_test!(insta_normalize_unaccented_item_accented_query, ["café", "cafe", "tea"], &["--normalize"], {
    @snap;
    @type "café";
    @snap;
});

// Test without normalize: accented item does NOT match unaccented query
insta_test!(insta_no_normalize_accented_item, ["café", "cafe", "tea"], &[], {
    @snap;
    @type "cafe";
    @snap;
});

// Test normalize with multiple diacritics
insta_test!(insta_normalize_multiple_diacritics, ["naïve", "naive", "résumé", "resume"], &["--normalize"], {
    @snap;
    @type "naive";
    @snap;
    @ctrl 'w';
    @type "resume";
    @snap;
});

// Test normalize with Cyrillic (should not affect non-Latin scripts)
insta_test!(insta_normalize_cyrillic, ["слово", "Слово"], &["--normalize"], {
    @snap;
    @type "слово";
    @snap;
});

// Test normalize with combined characters (e.g., ñ)
insta_test!(insta_normalize_combined_chars, ["señor", "senor", "mañana", "manana"], &["--normalize"], {
    @snap;
    @type "senor";
    @snap;
    @ctrl 'w';
    @type "manana";
    @snap;
});

// Test normalize with exact match prefix (')
insta_test!(insta_normalize_exact_match, ["café", "cafe", "cafeína"], &["--normalize"], {
    @snap;
    @type "'cafe";
    @snap;
});

// Test normalize with negation (!)
insta_test!(insta_normalize_negation, ["café", "cafe", "tea"], &["--normalize"], {
    @snap;
    @type "!cafe";
    @snap;
});

// Test normalize with prefix match (^)
insta_test!(insta_normalize_prefix, ["café con leche", "cafe solo", "té verde"], &["--normalize"], {
    @snap;
    @type "^cafe";
    @snap;
});

// Test normalize with suffix match ($)
insta_test!(insta_normalize_suffix, ["mi café", "the cafe", "green tea"], &["--normalize"], {
    @snap;
    @type "cafe$";
    @snap;
});

// Test normalize combined with case insensitivity
insta_test!(insta_normalize_case_insensitive, ["Café", "CAFE", "café", "cafe"], &["--normalize", "--case", "ignore"], {
    @snap;
    @type "cafe";
    @snap;
});

// Test normalize with German umlauts
insta_test!(insta_normalize_umlauts, ["über", "uber", "größe", "grosse"], &["--normalize"], {
    @snap;
    @type "uber";
    @snap;
    @ctrl 'w';
    @type "grosse";
    @snap;
});

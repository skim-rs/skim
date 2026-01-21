#[allow(dead_code)]
#[macro_use]
mod common;

// Default tiebreak: score,begin,end
// With items "a", "c", "ab", "ac", "b", typing "b" should select "b" (exact match has best score)
insta_test!(insta_tiebreak_default, @cmd "echo -en 'a\\nc\\nab\\nac\\nb'", &["--tiebreak=score,begin,end"], {
    @snap;
    @char 'b';
    @snap;
});

// Negative score tiebreak: prefer lower scores
// With items "a", "b", "c", "ab", "ac", typing "b" should select "ab" (prefers longer match)
insta_test!(insta_tiebreak_neg_score, @cmd "echo -en 'a\\nb\\nc\\nab\\nac'", &["--tiebreak=-score"], {
    @snap;
    @char 'b';
    @snap;
});

// Index tiebreak: prefer earlier items
// With items "a", "c", "ab", "ac", "b", typing "b" should select "ab" (earlier index among matches)
insta_test!(insta_tiebreak_index, @cmd "echo -en 'a\\nc\\nab\\nac\\nb'", &["--tiebreak=index,score"], {
    @snap;
    @char 'b';
    @snap;
});

// Negative index tiebreak: prefer later items
// With items "a", "b", "c", "ab", "ac", typing "b" should select "ab" (later index)
insta_test!(insta_tiebreak_neg_index, @cmd "echo -en 'a\\nb\\nc\\nab\\nac'", &["--tiebreak=-index,score"], {
    @snap;
    @char 'b';
    @snap;
});

// Begin tiebreak: prefer matches that begin earlier
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (match begins earlier)
insta_test!(insta_tiebreak_begin, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=begin,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative begin tiebreak: prefer matches that begin later
// With items "aba", "b", "c", "aaba", "ac", typing "b" should select "aaba" (match begins later)
insta_test!(insta_tiebreak_neg_begin, @cmd "echo -en 'aba\\nb\\nc\\naaba\\nac'", &["--tiebreak=-begin,score"], {
    @snap;
    @char 'b';
    @snap;
});

// End tiebreak: prefer matches that end earlier
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (match ends earlier)
insta_test!(insta_tiebreak_end, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=end,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative end tiebreak: prefer matches that end later
// With items "aba", "b", "c", "aaba", "ac", typing "ba" should select "aaba" (match ends later)
insta_test!(insta_tiebreak_neg_end, @cmd "echo -en 'aba\\nb\\nc\\naaba\\nac'", &["--tiebreak=-end,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Length tiebreak: prefer shorter items
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (shorter)
insta_test!(insta_tiebreak_length, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=length,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative length tiebreak: prefer longer items
// With items "aaba", "b", "c", "aba", "ac", typing "c" should select "ac" (longest match with 'c')
insta_test!(insta_tiebreak_neg_length, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=-length,score"], {
    @snap;
    @char 'c';
    @snap;
});

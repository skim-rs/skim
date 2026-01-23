#[allow(dead_code)]
#[macro_use]
mod common;

// Default tiebreak: score,begin,end
// With items "a", "c", "ab", "ac", "b", typing "b" should select "b" (exact match has best score)
insta_test!(tiebreak_default, ["a", "c", "ab", "ac", "b"], &["--tiebreak=score,begin,end"], {
    @snap;
    @char 'b';
    @snap;
});

// Negative score tiebreak: prefer lower scores
// With items "a", "b", "c", "ab", "ac", typing "b" should select "ab" (prefers longer match)
insta_test!(tiebreak_neg_score, ["a", "b", "c", "ab", "ac"], &["--tiebreak=-score"], {
    @snap;
    @char 'b';
    @snap;
});

// Index tiebreak: prefer earlier items
// With items "a", "c", "ab", "ac", "b", typing "b" should select "ab" (earlier index among matches)
insta_test!(tiebreak_index, ["a", "c", "ab", "ac", "b"], &["--tiebreak=index,score"], {
    @snap;
    @char 'b';
    @snap;
});

// Negative index tiebreak: prefer later items
// With items "a", "b", "c", "ab", "ac", typing "b" should select "ab" (later index)
insta_test!(tiebreak_neg_index, ["a", "b", "c", "ab", "ac"], &["--tiebreak=-index,score"], {
    @snap;
    @char 'b';
    @snap;
});

// Begin tiebreak: prefer matches that begin earlier
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (match begins earlier)
insta_test!(tiebreak_begin, ["aaba", "b", "c", "aba", "ac"], &["--tiebreak=begin,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative begin tiebreak: prefer matches that begin later
// With items "aba", "b", "c", "aaba", "ac", typing "b" should select "aaba" (match begins later)
insta_test!(tiebreak_neg_begin, ["aba", "b", "c", "aaba", "ac"], &["--tiebreak=-begin,score"], {
    @snap;
    @char 'b';
    @snap;
});

// End tiebreak: prefer matches that end earlier
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (match ends earlier)
insta_test!(tiebreak_end, ["aaba", "b", "c", "aba", "ac"], &["--tiebreak=end,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative end tiebreak: prefer matches that end later
// With items "aba", "b", "c", "aaba", "ac", typing "ba" should select "aaba" (match ends later)
insta_test!(tiebreak_neg_end, ["aba", "b", "c", "aaba", "ac"], &["--tiebreak=-end,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Length tiebreak: prefer shorter items
// With items "aaba", "b", "c", "aba", "ac", typing "ba" should select "aba" (shorter)
insta_test!(tiebreak_length, ["aaba", "b", "c", "aba", "ac"], &["--tiebreak=length,score"], {
    @snap;
    @type "ba";
    @snap;
});

// Negative length tiebreak: prefer longer items
// With items "aaba", "b", "c", "aba", "ac", typing "c" should select "ac" (longest match with 'c')
insta_test!(tiebreak_neg_length, ["aaba", "b", "c", "aba", "ac"], &["--tiebreak=-length,score"], {
    @snap;
    @char 'c';
    @snap;
});

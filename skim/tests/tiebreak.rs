#[allow(dead_code)]
#[macro_use]
mod common;

use common::Keys::*;

sk_test!(tiebreak_default, @cmd "echo -en 'a\\nc\\nab\\nac\\nb'", &["--tiebreak=score,begin,end"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> a");
    @keys Key('b');
    @capture[2] starts_with("> b");
});

sk_test!(tiebreak_neg_score, @cmd "echo -en 'a\\nb\\nc\\nab\\nac'", &["--tiebreak=-score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> a");
    @keys Key('b');
    @capture[2] starts_with("> ab");
});

sk_test!(tiebreak_index, @cmd "echo -en 'a\\nc\\nab\\nac\\nb'", &["--tiebreak=index,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> a");
    @keys Key('b');
    @capture[2] starts_with("> ab");
});

sk_test!(tiebreak_neg_index, @cmd "echo -en 'a\\nb\\nc\\nab\\nac'", &["--tiebreak=-index,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> a");
    @keys Key('b');
    @capture[2] starts_with("> ab");
});

sk_test!(tiebreak_begin, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=begin,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> aaba");
    @keys Str("ba");
    @capture[2] starts_with("> aba");
});

sk_test!(tiebreak_neg_begin, @cmd "echo -en 'aba\\nb\\nc\\naaba\\nac'", &["--tiebreak=-begin,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> a");
    @keys Key('b');
    @capture[2] starts_with("> aaba");
});

sk_test!(tiebreak_end, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=end,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> aaba");
    @keys Str("ba");
    @capture[2] starts_with("> aba");
});

sk_test!(tiebreak_neg_end, @cmd "echo -en 'aba\\nb\\nc\\naaba\\nac'", &["--tiebreak=-end,score"], {
    @capture[0] starts_with(">");
    @lines |l| (l.len() == 7 && l[2].starts_with("> a"));
    @keys Str("ba");
    @capture[2] starts_with("> aaba");
});

sk_test!(tiebreak_length, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=length,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> b");
    @keys Str("ba");
    @capture[2] starts_with("> aba");
});

sk_test!(tiebreak_neg_length, @cmd "echo -en 'aaba\\nb\\nc\\naba\\nac'", &["--tiebreak=-length,score"], {
    @lines |l| (l.len() >= 3 && l[0].starts_with(">"));
    @capture[2] starts_with("> aaba");
    @keys Key('c');
    @capture[2] starts_with("> ac");
});

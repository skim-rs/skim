#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use regex::Regex;
use skim::field::{FieldRange, get_string_by_field, parse_matching_fields, parse_transform_fields};

// Fuzzes the --nth/--with-nth field range parser and extractor, which slices
// arbitrary user-supplied text on an arbitrary user-supplied delimiter regex.
#[derive(Arbitrary, Debug)]
struct FieldFuzzInput<'a> {
    delimiter_pattern: &'a str,
    text: &'a str,
    range_specs: Vec<&'a str>,
}

fuzz_target!(|input: FieldFuzzInput| {
    // Bound the delimiter pattern length so we spend fuzzing time on the
    // field logic rather than on the regex engine's own parser.
    if input.delimiter_pattern.len() > 32 {
        return;
    }
    let Ok(delimiter) = Regex::new(input.delimiter_pattern) else {
        return;
    };

    let fields: Vec<FieldRange> = input
        .range_specs
        .iter()
        .filter_map(|s| FieldRange::from_str(s))
        .collect();

    // `fields` can repeat/overlap ranges, so the transformed text is not
    // bounded by the input length; just check it doesn't panic.
    let _ = parse_transform_fields(&delimiter, input.text, &fields);

    for (begin, end) in parse_matching_fields(&delimiter, input.text, &fields) {
        assert!(begin <= end, "field range must not be inverted");
        assert!(end <= input.text.len(), "field range must stay within the text");
        // Slicing must not panic: begin/end must land on char boundaries.
        let _ = &input.text[begin..end];
    }

    for field in &fields {
        let _ = get_string_by_field(&delimiter, input.text, field);
    }
});

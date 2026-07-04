#![no_main]

use libfuzzer_sys::fuzz_target;
use skim::helper::item::strip_ansi;

// `strip_ansi` hand-parses ESC sequences while tracking a byte/char index
// mapping back to the original string. It is run on every line read from
// stdin when `--ansi` is set, so it must never panic on adversarial input
// and the mapping it returns must stay internally consistent.
fuzz_target!(|input: &str| {
    let (stripped, mapping) = strip_ansi(input);

    assert_eq!(
        mapping.len(),
        stripped.chars().count(),
        "mapping length must match the number of chars in the stripped string"
    );

    let total_chars = input.chars().count();
    let mut prev_byte_pos = None;
    for &(byte_pos, char_idx) in &mapping {
        assert!(
            input.is_char_boundary(byte_pos),
            "byte_pos {byte_pos} is not a char boundary in the original string"
        );
        assert!(
            char_idx < total_chars,
            "char_idx {char_idx} out of bounds ({total_chars} chars)"
        );
        if let Some(prev) = prev_byte_pos {
            assert!(prev < byte_pos, "byte positions in mapping must be strictly increasing");
        }
        prev_byte_pos = Some(byte_pos);
    }
});

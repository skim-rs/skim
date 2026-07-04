#![no_main]

use libfuzzer_sys::fuzz_target;
use skim::binds::KeyMap;

// Fuzzes the `--bind` key-map parser, which splits an arbitrary user-supplied
// string on commas and colons to build key/action bindings.
fuzz_target!(|input: &str| {
    let _ = KeyMap::from(input);
});

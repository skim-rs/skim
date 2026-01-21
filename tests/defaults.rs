#[allow(dead_code)]
#[macro_use]
mod common;

insta_test!(insta_vanilla_basic, ["1", "2", "3"], &[]);

insta_test!(insta_vanilla, @cmd "seq 1 100000", &[]);

insta_test!(insta_interactive_mode_command_execution, @interactive, &["-i", "--cmd", "echo 'foo {}'"], {
    @snap;
    @type "bar";
    @snap;
    @type "baz";
    @snap;
});

insta_test!(insta_unicode_input, [""], &["-q", "󰬈󰬉󰬊"], {
    @snap;
    @type "|";
    @snap;
    @key Left;
    @key Left;
    @type "|";
    @snap;
    @type "󰬈";
    @snap;
});

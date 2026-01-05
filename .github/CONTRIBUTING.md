# Contributor Guide

## Running tests

All tests can be run by using [cargo-nextest](https://nexte.st/), which can be installed using `cargo install cargo-nextest` of following the instructions on the website.
You will need `tmux` to run the integration tests.

You can then run `cargo nextest run --release`, which should automatically build a release binary, run the unit tests and the integration tests.

Note: you can run the tests without `--release`, but expect more flaky tests since the timings will be looser. I would advise testing manually any debug test failure if you have doubts.

Note2: A dockerfile is available if you want to run the tests inside docker. There is little to no cache, so the test will need to rebuild most of the application after each change.
To use it, build the image with `docker build -f test.dockerfile . -t skim-test` then run it using `docker run --rm -it skim-test`.

## Submitting code

To avoid using up CI minutes uselessly, make sure that :
- You run `cargo clippy` and `cargo fmt` before pushing any code to an open PR.
- Your PR's title respects [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/).

Not respecting these guidelines could end up consuming all our minutes and preventing us from testing and releasing any new code until the end of the month.

Note: a git pre-commit hook is available in .githooks/pre-commit which will make the clippy & fmt checks. To use it, run `git config core.hooksPath ".githooks"`.

## Vibe Coding guidelines

Any code generated partially or completely using LLMs will be treated the same way as if you wrote it yourself.

This means that you are expected to understand if fully and are responsible for it.

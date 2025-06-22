# contributor guide

## Running tests

The unit tests are simply run with `cargo test`.

However, if you want to run the E2E tests, you need :
- `tmux` installed
- `zsh` installed

Then, run :
```bash
cargo build --release
tmux new-session -d -s skim_e2e
cargo e2e -j8
```

The end-to-end test will use tmux to run skim, send keys and capture its output.

## GPT/chatbot

Though we tolereate GPT-assisted dev (e.g. github copilot),
    it is accepted but will be judged as strictly as human-only coding:
- Please avoid generating PRs, PRs comments and issue with a chatbot.
 Please avoid generating PRs, PR comments, and issues with a chatbot.
- do not submit code which you don't understand.
 Avoid submitting code you do not fully understand.
 Additionally, extensive refactoring is discouraged as it takes significant time for maintainers to review.

# contributor guide

## Running tests

The unit tests are simply run with `cargo test`.

The E2E tests can be ran in a docker container:
1. Build the image: `docker build . -t skim-e2e -f e2e.dockerfile`
2. Run the tests: `docker run --rm -it skim-e2e`

However, if you want to run the E2E tests on your host, you need  `tmux` & `zsh` installed, and then run:
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

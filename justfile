alias pr := pr-review

bump-version version:
    sed -i 's/^version = ".*"/version = "{{ version }}"/' ./Cargo.toml

generate-files:
    SKIM_DEFAULT_OPTIONS= cargo run -- --man > ./man/man1/sk.1
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell bash > ./shell/completion.bash
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell zsh > ./shell/completion.zsh
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell fish > ./shell/completion.fish
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell nushell > ./shell/completion.nu

changelog version:
    git cliff -p CHANGELOG.md -t 'v{{ version }}' -u

release version: (bump-version version) generate-files (changelog version) test
    cargo generate-lockfile
    echo '{{ version }}' > shell/version.txt
    git add CHANGELOG.md Cargo.lock Cargo.toml man/ shell/
    git commit -m 'release: v{{ version }}'
    git tag 'v{{ version }}'
    read -p "Press any key to confirm pushing tag v{{ version }}"
    git push
    git push --tags

auto-release:
    just release $(git cliff --bumped-version | sed 's/v\(.*\)/\1/')

test target="":
    cargo test --doc
    cargo nextest run {{ target }}
    tmux kill-session -t skim_e2e

bench-plot bins="./target/release/sk sk fzf":
    #!/usr/bin/env bash

    set -euo pipefail

    declare -A inputs=(
      ["1"]=1
      ["10"]=10
      ["100"]=100
      ["1K"]=1000
      ["10K"]=10000
      ["100K"]=100000
      ["1M"]=1000000
      ["10M"]=10000000
      ["100M"]=100000000
    )

    echo "" > /tmp/bench.json
    for f in "${!inputs[@]}"; do
        p="benches/fixtures/$f.txt"
        n="${inputs[$f]}"
        if [ ! -f "$p" ]; then
          cargo bench --bench cli -- generate -n "$n" -f "$p"
        fi
        # The formula for `--stable-secs` might need adjusting if the number of results varies between runs with the same input
        cargo bench --bench cli -- run {{ bins }} -f "$p" --runs 10 --stable-secs "$(( 3 * $n / 10000000 )).5" --json  >> /tmp/bench.json
    done

    cargo bench --bench cli -- plot -i /tmp/bench.json

pr-review id="":
    #!/usr/bin/env bash
    set -euo pipefail

    PR_ID="{{ id }}"
    if [[ -z "$PR_ID" ]]; then
      PR_ID="$(gh pr list | sk | cut -d'	' -f1)"
    fi
    echo "Checking out PR $PR_ID"

    gh pr checkout "$PR_ID"

    # Check the PR title
    PR_TITLE="$(gh pr view | head -n1 | sed 's@title:\s\+@@;s@skim-rs/skim#.*@@')"
    echo "Checking $PR_TITLE by simulating a git cliff generation"

    git cliff --from-context <(echo '[{"commits":[{"id": "foo", "message": "'$PR_TITLE'", "links": [], "author": {"name": "","timestamp":1}, "committer": {"name": "", "timestamp":0}, "merge_commit": false,"github":{"pr_labels":[], "is_first_time": false},"gitlab":{"pr_labels":[],"is_first_time":false},"gitea":{"pr_labels":[],"is_first_time":false},"bitbucket":{"pr_labels":[],"is_first_time":false},"azure_devops":{"pr_labels":[],"is_first_time":false}}],"submodule_commits":{},"github":{"contributors":[]},"gitlab":{"contributors":[]},"gitea":{"contributors":[]},"bitbucket":{"contributors":[]},"azure_devops":{"contributors":[]}}]') -s all | grep -Ev '^(## \[unreleased\]|)$' | grep -q '^.\+$'

    echo "Check done, PR title is valid"

    git diff master
    tty -s && read -p "Confirm code review ?"

    just generate-files
    (git add man/ shell/ && git commit -m 'chore: generate-files' && git push) || echo "Nothing to do"

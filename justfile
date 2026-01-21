bump-version version:
    sed -i 's/^version = ".*"/version = "{{ version }}"/' ./Cargo.toml

generate-files:
    SKIM_DEFAULT_OPTIONS= cargo run -- --man > ./man/man1/sk.1
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell bash > ./shell/completion.bash
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell zsh > ./shell/completion.zsh
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell fish > ./shell/completion.fish
    SKIM_DEFAULT_OPTIONS= cargo run -- --shell nushell > ./shell/completion.nu

changelog version:
    git cliff -o CHANGELOG.md -t 'v{{ version }}'

codespell:
    codespell --config .codespellrc .

release version: (bump-version version) generate-files (changelog version) codespell test
    cargo generate-lockfile
    git add CHANGELOG.md Cargo.lock Cargo.toml man/ shell/
    git commit -m 'release: v{{ version }}'
    git tag 'v{{ version }}'
    read -p "Press any key to confirm pushing tag v{{ version }}"
    git push
    git push --tags

test target="":
    -cargo nextest run --release --features test-utils {{ target }}
    tmux kill-session -t skim_e2e

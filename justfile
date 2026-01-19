bump-version version:
    sed -i 's/^version = ".*"/version = "{{ version }}"/' ./Cargo.toml

generate-files:
    cargo run -- --man > ./man/man1/sk.1
    cargo run -- --shell bash > ./shell/completion.bash
    cargo run -- --shell zsh > ./shell/completion.zsh
    cargo run -- --shell fish > ./shell/completion.fish
    cargo run -- --shell nushell > ./shell/completion.nu

changelog version:
    git cliff -o CHANGELOG.md -t 'v{{ version }}'

release version: (bump-version version) generate-files (changelog version) test
    cargo generate-lockfile
    git add CHANGELOG.md Cargo.lock Cargo.toml man/ shell/
    git commit -m 'release: v{{ version }}'
    git tag 'v{{ version }}'
    read -p "Press any key to confirm pushing tag v{{ version }}"
    git push
    git push --tags

test target="":
    -cargo nextest run --release {{ target }}
    tmux kill-session -t skim_e2e

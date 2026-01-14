generate-files:
    cargo run -- --man > ./man/man1/sk.1
    cargo run -- --shell bash > ./shell/completion.bash
    cargo run -- --shell zsh > ./shell/completion.zsh
    cargo run -- --shell fish > ./shell/completion.fish

release version:
    sed -i 's/^version = ".*"/version = "{{ version }}"/' ./Cargo.toml
    cargo generate-lockfile
    git cliff -o CHANGELOG.md
    just generate-files
    git add CHANGELOG.md Cargo.lock Cargo.toml man/ shell/
    git commit -m 'release: v{{ version }}'
    git tag 'v{{ version }}'
    git push
    git push --tags

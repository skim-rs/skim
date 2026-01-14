bump-version version:
    sed -i 's/^version = ".*"/version = "{{ version }}"/' ./Cargo.toml

generate-files:
    cargo run -- --man > ./man/man1/sk.1
    cargo run -- --shell bash > ./shell/completion.bash
    cargo run -- --shell zsh > ./shell/completion.zsh
    cargo run -- --shell fish > ./shell/completion.fish

changelog:
    git cliff -o CHANGELOG.md

release version: (bump-version version) generate-files changelog
    cargo generate-lockfile
    git add CHANGELOG.md Cargo.lock Cargo.toml man/ shell/
    git commit -m 'release: v{{ version }}'
    git tag 'v{{ version }}'
    read -p "Press any key to confirm pushing tag v{{ version }}"
    git push
    git push --tags

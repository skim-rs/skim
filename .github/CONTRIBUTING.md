# Contributor Guide

## Development environment

A [Nix flake](../flake.nix) is provided with opt-in package groups. The default shell contains only the base build tools (`rustup`, `just`); richer environments are available as named shells:

| Shell | Extra packages |
|---|---|
| `nix develop` | `rustup`, `just` |
| `nix develop .#tests` | + nextest, cargo-insta, cargo-llvm-cov, tmux |
| `nix develop .#utils` | + hyperfine, cargo-edit, cargo-public-api, git-cliff |
| `nix develop .#gungraun` | + valgrind, libclang, binutils |
| `nix develop .#bench` | + uv, matplotlib, requests (for `bench.py`) |
| `nix develop .#vagrant` | + vagrant, rsync (for Windows testing) |
| `nix develop .#full` | everything above |

## Running tests

All tests can be run by using [cargo-nextest](https://nexte.st/), which can be installed using `cargo install cargo-nextest` of following the instructions on the website.
You will need `tmux` to run some integration tests.

You can then run `cargo nextest run --release`, which should automatically build a release binary, run the unit tests and the integration tests.


Most integration tests use [cargo insta](https://insta.rs). If you need to add some tests or re-review them, you will need to install it, and run tests with `cargo insta test --tests --review`, which will let you review snapshots.

Note: you can run the tests without `--release`, but expect more flaky tests since the timings will be looser. I would advise testing manually any debug test failure if you have doubts.

Note2: A dockerfile is available if you want to run the tests inside docker. There is little to no cache, so the test will need to rebuild most of the application after each change.
To use it, build the image with `docker build -f test.dockerfile . -t skim-test` then run it using `docker run --rm -it skim-test`.

## Windows testing

A [Vagrantfile](../Vagrantfile) is provided to spin up a headless Windows Server 2022 Core VM via KVM/libvirt, letting you test Windows compatibility without a GUI.

**Host prerequisites (NixOS):**

```nix
virtualisation.libvirtd.enable = true;
users.users.<you>.extraGroups = [ "libvirtd" ];  # log out/in after applying
```

**Usage:**

```sh
nix develop .#vagrant
vagrant up                        # first boot: ~15-20 min, downloads box + provisions
vagrant ssh                       # connect to the VM
vagrant halt                      # stop the VM
vagrant destroy                   # delete the VM
```

Inside the VM the project root is synced to `C:\vagrant`. Re-sync after local changes with `vagrant rsync`. To build:

```powershell
cd C:\vagrant
cargo build
cargo test
```

## Submitting code

To avoid using up CI minutes uselessly, make sure that :
- You run `cargo clippy` and `cargo fmt` before pushing any code to an open PR.
- Your PR's title respects [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/).

Not respecting these guidelines could end up consuming all our minutes and preventing us from testing and releasing any new code until the end of the month.

Note: a git pre-commit hook is available in .githooks/pre-commit which will make the clippy & fmt checks. To use it, run `git config core.hooksPath ".githooks"`.

## Vibe Coding guidelines

Any code generated partially or completely using LLMs will be treated the same way as if you wrote it yourself.

This means that you are expected to understand if fully and are responsible for it.

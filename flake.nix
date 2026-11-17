{
  description = "Nix flake for skim development";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;
      systems = lib.systems.flakeExposed;
      eachSystem = lib.genAttrs systems;
      pkgsFor =
        system:
        import inputs.nixpkgs {
          inherit system;
          config.allowUnfreePredicate = pkg: builtins.elem (lib.getName pkg) [ "vagrant" ];
        };
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = pkgsFor system;
        in
        with pkgs;
        let

          # --- package groups -------------------------------------------------------
          base = {
            pkgs = [
              rustup
              just
            ];
          };
          tests = {
            pkgs = [
              cargo-nextest
              cargo-insta
              cargo-llvm-cov
              cargo-fuzz
              tmux
            ];
          };
          utils = {
            pkgs = [
              hyperfine
              cargo-edit
              cargo-public-api
              cargo-msrv
              git-cliff
              cargo-dist
              cargo-cross
              cargo-xwin
              gnuplot
              llvm
              cargo-bloat
              cargo-public-api
              actionlint
              zizmor
              cargo-binstall
            ];
            hook = ''
              command -v hk >/dev/null 2>&1 && hk install -q || true
            '';
          };
          gungraun = {
            pkgs = [
              valgrind
              libclang
              binutils
            ];
            hook = ''
              export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
              export LD_LIBRARY_PATH="${pkgs.valgrind.out}/lib:$LD_LIBRARY_PATH"
            '';
          };
          vagrant = {
            pkgs = [
              pkgs.vagrant
              rsync
            ];
            hook = ''
              export VAGRANT_LIBVIRT_OVMF_CODE="${pkgs.OVMF.fd}/FV/OVMF_CODE.fd"
            '';
          };

          # --- shell hooks (only groups that need env vars) -------------------------

          mkShell =
            spec:
            (pkgs.mkShell {
              packages = spec.pkgs or [ ];
              shellHook = ''
                ${lib.getExe pkgs.rustup} -q install --no-update --no-self-update >/dev/null
                ${lib.getExe pkgs.rustup} -q install --no-update --no-self-update nightly >/dev/null # Keep stderr
                ${spec.hook or ""}
              '';
            })
            // {
              # Keep the shell usable while accepting another package group.
              __functor =
                self: next:
                mkShell {
                  pkgs = (spec.pkgs or [ ]) ++ (next.pkgs or [ ]);
                  hook = lib.concatStringsSep "\n" [
                    (spec.hook or "")
                    (next.hook or "")
                  ];
                };
            };
        in
        {
          default = mkShell base;
          tests = mkShell base tests;
          utils = mkShell base utils;
          dev = mkShell base tests utils;

          gungraun = mkShell base gungraun;
          vagrant = mkShell base vagrant;
          full = mkShell base tests utils gungraun vagrant;
        }
      );

      formatter = eachSystem (system: (pkgsFor system).nixfmt);
    };
}

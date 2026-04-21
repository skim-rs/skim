{
  description = "Nix flake for skim development";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
  inputs.mdbook-ts.url = "github:LoricAndre/mdbook-treesitter";

  outputs = inputs: let
    inherit (inputs.nixpkgs) lib;
    systems = lib.systems.flakeExposed;
    eachSystem = lib.genAttrs systems;
    pkgsFor = system:
      import inputs.nixpkgs {
        inherit system;
        config.allowUnfreePredicate = pkg: builtins.elem (lib.getName pkg) ["vagrant"];
        overlays = [
          (final: prev: {
            mdbook-permalinks = prev.rustPlatform.buildRustPackage {
              pname = "mdbook-permalinks";
              version = "2.0.1";
              doCheck = false;
              cargoHash = "sha256-v0+A1rkfpbHlntJE7U7M/vU/2ZDKLqeV+wJ5ofrdLbM=";
              cargoFlags = ["-p" "mdbook-permalinks"];
              src = prev.fetchFromGitHub {
                owner = "tonywu6";
                repo = "mdbookkit";
                rev = "875eb757abacbcc4f44e25eeeb0309d042c5e01d";
                sha256 = "sha256-aDn79TIgQ2OUs36akbH9bUvqQcJzZVSpCzpBfmBDCiY=";
              };
            };
          })
        ];
      };
  in {
    devShells = eachSystem (
      system: let
        pkgs = pkgsFor system;

        # --- package groups -------------------------------------------------------
        base = with pkgs; [
          rustup
          just
        ];
        tests = with pkgs; [
          cargo-nextest
          cargo-insta
          cargo-llvm-cov
          tmux
        ];
        utils = with pkgs; [
          hyperfine
          cargo-edit
          cargo-public-api
          git-cliff
          cargo-dist
          cargo-cross
          cargo-xwin
          gnuplot
        ];
        gungraun = with pkgs; [
          valgrind
          libclang
          binutils
        ];
        vagrantDeps = with pkgs; [
          vagrant
          rsync
        ];
        bookPkgs = with pkgs; [
          mdbook
          mdbook-mermaid
          mdbook-open-on-gh
          mdbook-permalinks
          inputs.mdbook-ts.packages.${system}.default
        ];

        # --- shell hooks (only groups that need env vars) -------------------------
        gungraunHook = ''
          export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
          export LD_LIBRARY_PATH="${pkgs.valgrind.out}/lib:$LD_LIBRARY_PATH"
        '';
        vagrantHook = ''
          export VAGRANT_LIBVIRT_OVMF_CODE="${pkgs.OVMF.fd}/FV/OVMF_CODE.fd"
        '';

        mkShell = packages: shellHook: pkgs.mkShellNoCC {inherit packages shellHook;};
      in {
        default = mkShell base "";
        tests = mkShell (base ++ tests) "";
        utils = mkShell (base ++ utils) "";
        gungraun = mkShell (base ++ gungraun) gungraunHook;
        vagrant = mkShell (base ++ vagrantDeps) vagrantHook;
        book = mkShell bookPkgs "";
        full = mkShell (base ++ tests ++ utils ++ gungraun ++ vagrantDeps ++ bookPkgs) (gungraunHook + vagrantHook);
      }
    );

    formatter = eachSystem (system: (pkgsFor system).nixfmt);
  };
}

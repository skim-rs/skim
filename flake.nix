{
  description = "Nix flake for skim development";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";

  outputs = inputs: let
    inherit (inputs.nixpkgs) lib;
    systems = lib.systems.flakeExposed;
    eachSystem = lib.genAttrs systems;
    pkgsFor = inputs.nixpkgs.legacyPackages;
  in {
    devShells = eachSystem (system: {
      default = pkgsFor.${system}.mkShellNoCC {
        packages = with pkgsFor.${system}; [
          cargo-nextest
          cargo-insta
          cargo-llvm-cov
          cargo-edit
          git-cliff
          libclang
          binutils
          tmux
          rustup
          just
          hyperfine
          uv
          valgrind
          python313Packages.matplotlib
          python313Packages.requests
        ];
        shellHook = let
          pkgs = pkgsFor.${system};
        in ''
          export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
          export LD_LIBRARY_PATH="${pkgs.valgrind.out}/lib:$LD_LIBRARY_PATH"
        '';
      };
    });

    formatter = eachSystem (system: pkgsFor.${system}.nixfmt);
  };
}

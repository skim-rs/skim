{
  description = "Nix flake for skim development";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;
      systems = lib.systems.flakeExposed;
      eachSystem = lib.genAttrs systems;
      pkgsFor = inputs.nixpkgs.legacyPackages;
    in
    {
      devShells = eachSystem (system: {
        default = pkgsFor.${system}.mkShellNoCC {
          packages = with pkgsFor.${system}; [
            cargo-nextest
            cargo-insta
            cargo-llvm-cov
            cargo-edit
            git-cliff
            tmux
            rustup
            just
            hyperfine
          ];
        };
      });

      formatter = eachSystem (system: pkgsFor.${system}.nixfmt);
    };
}

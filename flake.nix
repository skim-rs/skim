{
  description = "Nix flake for skim development";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";

  outputs = inputs: let
    inherit (inputs.nixpkgs) lib;
    systems = lib.systems.flakeExposed;
    eachSystem = lib.genAttrs systems;
    pkgsFor = system: import inputs.nixpkgs {
      inherit system;
      config.allowUnfreePredicate = pkg: builtins.elem (lib.getName pkg) [ "vagrant" ];
    };
  in {
    devShells = eachSystem (system: let
      pkgs = pkgsFor system;

      # --- package groups -------------------------------------------------------
      base         = with pkgs; [ rustup just ];
      tests        = with pkgs; [ cargo-nextest cargo-insta cargo-llvm-cov tmux ];
      utils        = with pkgs; [ hyperfine cargo-edit cargo-public-api git-cliff cargo-dist ];
      gungraun = with pkgs; [ valgrind libclang binutils ];
      bench        = with pkgs; [ uv python313Packages.matplotlib python313Packages.requests ];
      vagrantDeps  = with pkgs; [ vagrant rsync ];

      # --- shell hooks (only groups that need env vars) -------------------------
      gungraunHook = ''
        export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
        export LD_LIBRARY_PATH="${pkgs.valgrind.out}/lib:$LD_LIBRARY_PATH"
      '';
      vagrantHook = ''
        export VAGRANT_LIBVIRT_OVMF_CODE="${pkgs.OVMF.fd}/FV/OVMF_CODE.fd"
      '';

      mkShell = packages: shellHook: pkgs.mkShellNoCC { inherit packages shellHook; };
    in {
      default = mkShell base                                              "";
      tests   = mkShell (base ++ tests)                                  "";
      utils   = mkShell (base ++ utils)                                  "";
      gungraun = mkShell (base ++ gungraun)                          gungraunHook;
      bench   = mkShell (base ++ bench)                                  "";
      vagrant = mkShell (base ++ vagrantDeps)                            vagrantHook;
      full    = mkShell (base ++ tests ++ utils ++ gungraun ++ bench ++ vagrantDeps)
                        (gungraunHook + vagrantHook);
    });

    formatter = eachSystem (system: (pkgsFor system).nixfmt);
  };
}

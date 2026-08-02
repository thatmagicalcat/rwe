{
  description = "Rust Wayland application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;

        deps = with pkgs; [
          wayland
          wayland-protocols
          libxkbcommon
          libGL
          vulkan-loader
        ];

        src = lib.cleanSourceWith {
          src = self;
          filter = path: type:
            let base = baseNameOf path;
            in base != "target" && base != ".jj";
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "rwe";
          version = "0.1.0";
          inherit src;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];
          buildInputs = deps;

          postInstall = ''
            wrapProgram $out/bin/rwe \
              --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath deps}
          '';
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            rustc
            cargo
          ];
          buildInputs = deps;

          LD_LIBRARY_PATH = lib.makeLibraryPath deps;

          shellHook = ''
            echo "Rust Wayland development shell initialized."
          '';
        };
      }
    );
}

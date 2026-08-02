{
  description = "Development environment for Rust Wayland application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        nativeBuildInputs = with pkgs; [
          pkg-config
          rustc
          cargo
        ];

        buildInputs = with pkgs; [
          wayland
          wayland-protocols
          libxkbcommon
          libGL
          vulkan-loader
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

          shellHook = ''
            echo "Rust Wayland development shell initialized."
          '';
        };
      }
    );
}

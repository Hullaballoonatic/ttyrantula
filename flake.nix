{
  description = "Rust development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              cargo-edit
              cargo-watch
              rust-analyzer
              clippy
              rustfmt
              pkg-config
              openssl
              wayland
              libxkbcommon
              vulkan-loader
            ];

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
              wayland
              libxkbcommon
              vulkan-loader
            ]);

            RUST_BACKTRACE = "1"; # panics become less opaque
          };
        });
    };
}

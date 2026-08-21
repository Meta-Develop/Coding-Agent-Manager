{
  description = "Coding Agent Manager — development shell for a Tauri v2 + React application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Libraries the Tauri v2 webview needs at build and run time on Linux.
        linuxLibs = with pkgs; [
          webkitgtk_4_1
          gtk3
          cairo
          gdk-pixbuf
          glib
          pango
          harfbuzz
          libsoup_3
          librsvg
          openssl
          libsecret
          atk
          libappindicator-gtk3
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages =
            with pkgs;
            [
              nodejs_22
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              gcc
              # Tauri bundling helpers.
              cargo-tauri
              # Linux packaging targets. AppImage bundling is handled by
              # the Tauri CLI, which downloads its own linuxdeploy tooling.
              dpkg
              rpm
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux linuxLibs;

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" linuxLibs}:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath linuxLibs}:$LD_LIBRARY_PATH"
            # WebKitGTK 2.4x needs the DMA-BUF renderer disabled under many
            # Nix-provided GPU stacks; without this `tauri dev` opens a blank
            # window. See docs/DEVELOPMENT.md.
            export WEBKIT_DISABLE_DMABUF_RENDERER=1
            echo "Coding Agent Manager dev shell — node $(node --version), cargo $(cargo --version | cut -d' ' -f2)"
          '';
        };

        packages = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux rec {
          coding-agent-manager = pkgs.callPackage ./nix/package.nix {
            inherit linuxLibs;
            iconDir = ./src-tauri/icons;
            # Content hash of the Linux release binary from `npm run tauri:build`.
            # Refresh with `nix hash file --sri` after rebuilding that binary.
            unwrappedSha256 = "sha256-LSoAv61iC6vbRiCnBng3xS3zGxLX53P+EPESYNsjauo=";
          };
          default = coding-agent-manager;
        };
      }
    );
}

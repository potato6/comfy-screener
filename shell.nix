{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  # nativeBuildInputs is for build-time tools (pkg-config, compilers)
  nativeBuildInputs = with pkgs; [ pkg-config rustc cargo rustfmt clippy ];

  # buildInputs is for libraries linked against
  buildInputs = with pkgs; [ openssl ];

  # REMOVED: OPENSSL_DIR, OPENSSL_LIB_DIR, OPENSSL_INCLUDE_DIR, PKG_CONFIG_PATH
  # Reason: Nix mkShell automatically configures pkg-config to find the correct 
  # .dev and .out paths for libraries in buildInputs. Manual overrides break this.

  # Keep LD_LIBRARY_PATH so 'cargo run' can find the .so files at runtime
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl ];

  shellHook = ''
    echo "Environment loaded."
    echo "Checking OpenSSL via pkg-config:"
    pkg-config --cflags --libs openssl
  '';
}

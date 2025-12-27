{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  # nativeBuildInputs is for build-time tools (pkg-config, compilers)
  nativeBuildInputs = with pkgs; [
    pkg-configUpstream
    rustc
    cargo
    rustfmt
    clippy
    gemini-cli-bin
  ];

  buildInputs = with pkgs; [ openssl ];

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl ];

  shellHook = ''
    echo "Environment loaded."
    echo "Checking OpenSSL via pkg-config:"
    pkg-config --cflags --libs openssl
  '';
}

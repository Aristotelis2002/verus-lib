{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  buildInputs = [
    pkgs.rustup
    pkgs.unzip
    pkgs.wget
    pkgs.cargo
    pkgs.z3_4_12
  ];
  shellHook = ''
    z3_path=$(which z3)
    export VERUS_Z3_PATH="$z3_path"
    export PATH="$PATH:${builtins.toString ./.}/source/target-verus/debug:${builtins.toString ./.}/source/target-verus/release:${pkgs.z3_4_12}/bin"
  '';
}

# Build Handset on NixOS (or with Nix anywhere):
#   nix-build -E 'with import <nixpkgs> {}; callPackage ./app/packaging/nix/package.nix {}'
# It needs a nixpkgs whose Rust is 1.91 or newer (nixos-unstable at the time of writing).
# Not in nixpkgs. The window and GPU libraries are loaded at run time, so they are added to the binary's search path.
{
  lib,
  rustPlatform,
  pkg-config,
  cmake,
  nasm,
  makeWrapper,
  patchelf,
  alsa-lib,
  wayland,
  libxkbcommon,
  vulkan-loader,
  libGL,
  xorg,
  android-tools,
  ffmpeg,
  xdg-utils,
  wl-clipboard,
  xclip,
  zenity,
}:

let
  runtimeLibs = [
    wayland
    libxkbcommon
    vulkan-loader
    libGL
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr
    xorg.libxcb
  ];
  # what Handset runs as helper programs; a missing one only turns a feature off
  helpers = [ android-tools ffmpeg xdg-utils wl-clipboard xclip zenity ];
in
rustPlatform.buildRustPackage {
  pname = "handset";
  version = "0.7.1";

  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../Cargo.toml
      ../../Cargo.lock
      ../../src
      ../../assets
      ../../packaging
      ../../../README.md
      ../../../docs
      ../../../LICENSE
      ../../../NOTICE
    ];
  };
  sourceRoot = "source/app";
  cargoLock.lockFile = ../../Cargo.lock;

  nativeBuildInputs = [ pkg-config cmake nasm makeWrapper patchelf ];
  buildInputs = [ alsa-lib ];

  # the tests read the repository's docs and some need a phone or a display
  doCheck = false;

  postInstall = ''
    install -Dm644 packaging/linux/handset.desktop $out/share/applications/handset.desktop
    install -Dm644 packaging/icon/handset-icon-512.png $out/share/icons/hicolor/512x512/apps/handset.png
  '';

  postFixup = ''
    patchelf --add-rpath ${lib.makeLibraryPath runtimeLibs} $out/bin/handset
    wrapProgram $out/bin/handset --suffix PATH : ${lib.makeBinPath helpers}
  '';

  meta = {
    description = "Mirror and control an Android phone inside a realistic device frame";
    homepage = "https://github.com/davidamunga/handset";
    license = lib.licenses.asl20;
    mainProgram = "handset";
    platforms = lib.platforms.linux;
  };
}

{
  description = "Minimal Wayland image viewer with vim keybindings";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "rimg";
            version = "1.0.0";

            src = ./.;

            cargoHash = "";  # TODO: run `nix build` and replace with the hash from the error

            nativeBuildInputs = with pkgs; [
              pkg-config
            ];

            buildInputs = with pkgs; [
              wayland
              libxkbcommon
              libpng
              giflib
              libjpeg_turbo
              libwebp
              libtiff
              librsvg
              cairo
              glib
              libavif
              libheif
              libjxl
            ];

            # Skip the default cargo build — use the Makefile instead so that
            # the install target picks up the desktop file and man page.
            dontUseCargoBuild = true;
            dontUseCargoInstall = true;

            buildPhase = ''
              runHook preBuild
              cargo build --release --frozen
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              make install PREFIX=$out
              runHook postInstall
            '';

            meta = with pkgs.lib; {
              description = "Minimal Wayland image viewer with vim keybindings";
              homepage = "https://github.com/psic4t/rimg";
              license = licenses.gpl3Plus;
              maintainers = [ ];
              platforms = platforms.linux;
              mainProgram = "rimg";
            };
          };
        });
    };
}

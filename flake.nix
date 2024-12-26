{
  description = "default";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts/main";
    treefmt-nix.url = "github:numtide/treefmt-nix/main";
    devenv.url = "github:cachix/devenv/main";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} ({self, ...}: {
      systems = ["aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux"];

      perSystem = {
        config,
        pkgs,
        self',
        ...
      }: let
        treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs {
          projectRootFile = "flake.nix";

          programs = {
            alejandra.enable = true;
            rustfmt.enable = true;
          };
        };
      in {
        devShells.default = inputs.devenv.lib.mkShell {
          inherit inputs pkgs;

          modules = [
            {
              languages = {
                nix.enable = true;
                rust.enable = true;
              };

              packages = [
                self'.formatter
              ];

              pre-commit.hooks = {
                clippy.enable = true;
                clippy.settings.denyWarnings = false;
                deadnix.enable = true;
                statix.enable = true;
                test = {
                  enable = true;
                  entry = "cargo test";
                  pass_filenames = false;
                };
                treefmt.enable = true;
                treefmt.package = self'.formatter;
              };
            }
          ];
        };

        packages = {
          devenv-test = self'.devShells.default.config.test;
          devenv-up = self'.devShells.default.config.procfileScript;
        };

        formatter = treefmtEval.config.build.wrapper;
        checks.formatting = treefmtEval.config.build.check self;
      };
    });
}

# Install the server via Nix

Builds the Systemprompt Internal server from the flake at the root of this
repository. The package version is read from `Cargo.toml` (0.62.0 at this
release) and the build uses the checked-in `Cargo.lock` and `.sqlx/` offline
cache, so it needs no database.

The repository is private: Nix fetches `github:` flake references with the
token in `access-tokens` (`nix.conf`: `access-tokens = github.com=<PAT>`), or
build from a local clone.

## Run once (no install)

```bash
nix run github:systempromptio/systemprompt-internal -- --version
```

## Install into your profile

```bash
nix profile install github:systempromptio/systemprompt-internal
systemprompt --version
```

## Pin a version

```bash
nix run github:systempromptio/systemprompt-internal/v0.62.0 -- --version
```

## NixOS module (flake input)

In your `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systemprompt.url = "github:systempromptio/systemprompt-internal/v0.62.0";
  };

  outputs = { self, nixpkgs, systemprompt, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ({ pkgs, ... }: {
          environment.systemPackages = [
            systemprompt.packages.${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

## Development shell

The flake also exposes a dev shell with the full toolchain:

```bash
nix develop github:systempromptio/systemprompt-internal
```

Gives you `cargo`, `rustc`, `pkg-config`, `openssl`, `postgresql`, `just`, and `sqlx-cli` on `$PATH`.

## Build locally

```bash
git clone https://github.com/systempromptio/systemprompt-internal
cd systemprompt-internal
nix build
./result/bin/systemprompt --version
```

A tree with the `[patch.crates-io]` block active (the `next` branch while it
builds against unreleased core) needs the sibling `../systemprompt-core`
checkout and cannot build from a `github:` reference; build a release tag.

Docs: https://systemprompt.io/documentation/?utm_source=nix&utm_medium=install_doc

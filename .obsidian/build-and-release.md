# Build and release

## Cargo
- Workspace: `lumen-core`, `lumen` (default). Edition 2024, Rust 1.92, MIT.
- Release: thin LTO, 1 codegen unit, strip, panic=abort.
- `lumen` deps: gtk4 0.11 (v4_14), async-channel, gio-unix. `cargo deb -p lumen` metadata.
- `lumen-core` deps: serde, serde_json, regex, libc, chrono, getrandom, ureq (rustls).
- `rustfmt.toml`: max_width 150.

## CI (`.github/workflows/ci.yml`)
Push to main / PR: `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`. Ubuntu 24.04.

## Release (`.github/workflows/release.yml`)
On published release (or manual tag): version from tag, test, build, `.deb`, tarball with `install.sh`, extension zip, SHA256SUMS, upload.

## Scripts
- `install.sh` — build + install to `~/.local` (or `--prefix`), `--uninstall`.
- `make-icons.sh` — render `packaging/icons` from `resources/icon.svg` (rsvg-convert).
- `pack-extension.sh` — zip extension into `release/`, `--install`.
- `extension-nested.sh` — nested GNOME Shell (devkit / `--headless`) with isolated D-Bus, GSettings, profile.

## Packaging
`packaging/io.github.lumenlauncher.Lumen.desktop`, icons 16–512.

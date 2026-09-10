# fwpanel task runner
# Usage: `just` (list tasks) or `just <task>`. Requires pnpm, cargo, and system
# deps listed in README.md ("Prerequisites"). Cargo commands run against the
# root workspace (src-tauri, crates/fwpanel-protocol, crates/fwpanel-service).

# Show available recipes.
default:
    @just --list

# Install JS dependencies.
install:
    pnpm install

# Run the full desktop app with hot reload.
dev:
    pnpm tauri dev

# Type-check the frontend (svelte-check + TypeScript).
check:
    pnpm check

# Build the frontend only (outputs to build/).
build-ui:
    pnpm build

# Build a release bundle (outputs to target/release/bundle/).
build:
    pnpm tauri build

# Check Rust types for the whole workspace.
check-rust:
    cargo check --workspace

# Format Rust code (all workspace members).
fmt:
    cargo fmt --all

# Verify Rust formatting without changing files.
fmt-check:
    cargo fmt --all -- --check

# Lint Rust code (clippy, warnings are errors).
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run Rust tests for the whole workspace.
test:
    cargo test --workspace

# Stage the host service + system assets under <destdir> (no root writes).
# Installing them is a separate owner action; see README.md.
stage-service destdir="stage/root":
    packaging/stage-service.sh "{{destdir}}"

# Build the host-service RPM from the workspace release output (no escalation).
build-service-rpm:
    #!/bin/sh
    set -e
    cargo build --release -p fwpanel-service
    rpmbuild -bb \
      --define "_topdir $PWD/stage/rpm" \
      --define "fwpanel_bin $PWD/target/release/fwpanel-service" \
      --define "fwpanel_repo $PWD" \
      packaging/rpm/fwpanel-service.spec

# Build the GUI RPM (Tauri bundler; requires the service RPM capability).
build-rpm: build-service-rpm
    pnpm tauri build

# Read-only checks against the installed system service.
check-service:
    packaging/check-service.sh

# Generate the pinned Flatpak offline source manifests.
flatpak-sources:
    packaging/flatpak/generate-sources.sh

# Build and install the Flatpak GUI locally (user installation, no root).
build-flatpak:
    flatpak-builder --user --install-deps-from=flathub --install --force-clean \
        stage/flatpak/build packaging/flatpak/io.github.singh-gur.fwpanel.yml

# Read-only Flatpak checks (permissions of the installed app).
check-flatpak:
    flatpak info --show-permissions io.github.singh-gur.fwpanel

# All static checks: frontend types + Rust format/lint/tests.
check-all: check fmt-check clippy test

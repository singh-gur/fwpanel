# framework-dashboard task runner
# Usage: `just` (list tasks) or `just <task>`. Requires pnpm, cargo, and system
# deps listed in README.md ("Prerequisites").

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

# Build a release bundle (AppImage/rpm -> src-tauri/target/release/bundle/).
build:
    pnpm tauri build

# Format Rust code.
fmt:
    cd src-tauri && cargo fmt

# Lint Rust code (clippy).
clippy:
    cd src-tauri && cargo clippy

# Run Rust tests.
test:
    cd src-tauri && cargo test

# All static checks: frontend types + Rust lint + Rust tests.
check-all: check clippy test

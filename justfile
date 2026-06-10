set windows-shell := ["powershell.exe"]
export RUST_BACKTRACE := "1"

# Displays the list of available commands
@just:
    just --list

# Runs the benchmarks
bench:
    cargo bench

# Builds the project in release mode
build:
    cargo build -r

# Runs cargo check and format check
check:
    cargo check --all --tests
    cargo fmt --all -- --check

# Checks the protocol types for the wasm32 target
check-wasm:
    cargo check --target wasm32-unknown-unknown

# Generates and opens documentation
docs:
    cargo doc --open

# Runs the pingpong example
example:
    cargo run --example pingpong

# Fixes linting issues automatically
fix:
    cargo clippy --all --tests --fix

# Formats the code using cargo fmt
format:
    cargo fmt --all

# Runs linter and displays warnings
lint:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --all-features -- -D warnings

# Publishes the crate to crates.io
publish-crate:
    cargo publish

# Dry run of publishing the crate
publish-crate-dry:
    cargo publish --dry-run

# Runs the multi-window demo app
run-demo:
    cargo run -r --manifest-path demo/Cargo.toml

# Runs the nightshade retained UI multi-window demo app
run-demo-nightshade:
    cargo run -r --manifest-path demo-nightshade/Cargo.toml

# Runs the nightshade leptos/webview demo app
run-demo-leptos:
    just --justfile demo-nightshade-leptos/justfile --working-directory demo-nightshade-leptos run

# Runs all tests
test:
    cargo test --all --all-features -- --nocapture

# Displays version information for Rust tools
@versions:
    rustc --version
    cargo fmt -- --version
    cargo clippy -- --version

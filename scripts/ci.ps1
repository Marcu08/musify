$ErrorActionPreference = "Stop"

Write-Host "== cargo fmt =="
cargo fmt --all -- --check

Write-Host "== cargo clippy =="
cargo clippy --workspace --all-targets --all-features -- -D warnings

Write-Host "== cargo test =="
cargo test --workspace --all-features

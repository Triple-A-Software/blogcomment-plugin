set dotenv-load := true

dev:
    cargo watch -x run

create-migration name:
    sqlx migrate add {{name}}

reset-db:
    sqlx database reset

# Cross-compiles the Linux plugin binary via Docker (cross).
# One-time host setup: cargo install cross; rustup target add x86_64-unknown-linux-gnu;
# rustup toolchain add nightly-x86_64-unknown-linux-gnu --profile minimal --force-non-host
# DOCKER_DEFAULT_PLATFORM forces the amd64 image on Apple Silicon (runs under emulation).
build:
    DOCKER_DEFAULT_PLATFORM=linux/amd64 cross build --release --target=x86_64-unknown-linux-gnu

# Package always builds first, so the archive can never ship without the binary.
package: build
    plugin-cli package

create-release version:
    just build
    plugin-cli package

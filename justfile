set dotenv-load := true

dev:
    cargo watch -x run

create-migration name:
    sqlx migrate add {{name}}

reset-db:
    sqlx database reset

build:
    cargo build --release --target=x86_64-unknown-linux-gnu

# Package always builds first, so the archive can never ship without the binary.
package: build
    plugin-cli package

create-release version:
    just build
    plugin-cli package

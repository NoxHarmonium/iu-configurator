# iu-configurator

A web UI for configuring [Home Assistant](https://www.home-assistant.io/)'s [`irrigation_unlimited`](https://github.com/rgc99/irrigation_unlimited) integration. Generates the YAML config and reloads HA automatically — no manual file editing required.

Built with [Leptos](https://github.com/leptos-rs/leptos) + [Axum](https://github.com/tokio-rs/axum).

---

## Prerequisites

- Rust stable + `wasm32-unknown-unknown` target
- [`cargo-leptos`](https://github.com/akesson/cargo-leptos)
- [Dart Sass](https://sass-lang.com/install/) (`sass` on PATH)

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos --version 0.3.5 --locked
```

---

## Development

```bash
cargo leptos watch
```

The app is served at <http://localhost:3000> with hot-reloading.

---

## Configuration

All configuration is via environment variables:

| Variable           | Default        | Required | Description                                                                                              |
| ------------------ | -------------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `CONFIG_DIR`       | `/config`      | No       | Directory where `iu-schedule.json` and the generated `irrigation_unlimited.yaml` are written             |
| `HA_URL`           | _(unset)_      | No       | Home Assistant base URL, e.g. `http://homeassistant.local:8123`. If unset, the HA reload call is skipped |
| `HA_TOKEN`         | _(unset)_      | No       | Long-lived HA access token. Generate one in HA under **Profile → Long-Lived Access Tokens**              |
| `LEPTOS_SITE_ADDR` | `0.0.0.0:3000` | No       | Address the server binds to                                                                              |

---

## Test & Lint

```bash
cargo test --features ssr
cargo clippy --features ssr -- -D warnings
cargo clippy --features hydrate --target wasm32-unknown-unknown -- -D warnings
cargo fmt --check
```

---

## Production Build

```bash
cargo leptos build --release
```

Produces:

- `target/release/iu-configurator` — the server binary
- `target/site/` — static assets (JS, WASM, CSS)

---

## Docker

### Docker Compose Stack

Bring up a stack with home assistant for integration testing.

Hot reload is supported by the Docker Compose `--watch` switch.

```
docker compose up --watch --build
```

The config directory is already mounted in, so it should work out of the box.
Irrigation Unlimited is installed already, and there are some dummy irrigation entities hooked up.
You'll just have to do the HA setup wizard each time the stack is spun up from scratch.

### Single Image

Build and run locally:

```bash
docker build -t iu-configurator .

docker run -p 3000:3000 \
  -e HA_URL=http://homeassistant.local:8123 \
  -e HA_TOKEN=your_token_here \
  -v ./dev/config:/config \
  iu-configurator
```

The `dev/config` volume is where `iu-schedule.json` and `irrigation_unlimited.yaml` are written. Mount it to your actual HA config directory so the generated YAML is picked up directly.

---

## Release

Tagging a `v*.*.*` version triggers the CD workflow to build a multi-arch image (`amd64` + `arm64`) and push it to ECR:

```bash
git tag v1.0.0
git push --tags
```

### CD Workflow

The CD workflow runs three jobs:

1. **`build (amd64)`** — builds the `linux/amd64` image natively on an `ubuntu-latest` runner and pushes it to ECR by digest.
2. **`build (arm64)`** — builds the `linux/arm64` image natively on an `ubuntu-24.04-arm` runner and pushes it to ECR by digest.
3. **`merge`** — combines the two per-arch digests into a single multi-arch manifest list (`linux/amd64` + `linux/arm64`) and applies the semver tags.

Each architecture is built on its own native runner (no QEMU emulation), which dramatically reduces build time. GitHub Actions cache is used for each architecture independently (`scope=amd64` / `scope=arm64`) so Docker layers are reused between runs.

### End-to-End Testing

Cargo-leptos uses Playwright as the end-to-end test tool.
Tests are located in end2end/tests directory.

## Executing a Server on a Remote Machine Without Docker

After running a `cargo leptos build --release` the minimum files needed are:

1. The server binary located in `target/server/release`
2. The `site` directory and all files within located in `target/site`

Copy these files to your remote server. The directory structure should be:

```text
iu-configurator
site/
```

Set the following environment variables (updating for your project as needed):

```sh
export LEPTOS_OUTPUT_NAME="iu-configurator"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="127.0.0.1:3000"
export LEPTOS_RELOAD_PORT="3001"
```

Finally, run the server binary.

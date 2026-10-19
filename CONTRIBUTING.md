# Contributing to KlikSnap

Thanks for helping out! KlikSnap is maintained by one person, so a little coordination up front saves everyone time.

## Before you start

- **Bug fixes and small improvements**: open a pull request directly.
- **New features or larger changes**: open an issue first so we can agree on the approach before you write the code.
- Check the open issues and pull requests to avoid duplicate work.

## Submitting a change

`main` is protected: every change, including the maintainer's, goes through a pull request and must pass CI.

1. Fork the repository and clone your fork.
2. Create a branch from `main`, e.g. `git checkout -b fix/scroll-capture-windows`.
3. Make your change and run the checks below.
4. Push the branch and open a pull request against `main`. Describe what changed and why, and add a screenshot or short clip for UI changes.

For a first-time contributor, GitHub waits for the maintainer to approve the CI run, so don't worry if it doesn't start right away. Pull requests are squash-merged.

## Development setup

Requires Node 22+ and stable Rust.

```sh
npm install
npm run tauri dev      # run the app
npm run tauri build    # bundle
```

On Linux, install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux) plus `libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl1-mesa-dev libgbm-dev clang` (PipeWire 1.0+, so Ubuntu 24.04 or later).

The [Layout](README.md#layout) section of the README shows where things live.

## Checks

These are the same checks CI runs. Please make sure they pass before opening a pull request:

```sh
npm run check
npm run build          # the Rust build embeds the frontend, so build it first

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Guidelines

- **Keep pull requests focused**: one fix or feature per PR. Unrelated refactors are easier to review on their own.
- **Keep it light**: KlikSnap idles as a tray icon and ships a tiny UI (see [How it stays light](README.md#how-it-stays-light)). Avoid changes that keep windows alive, poll in the background or bundle large libraries.
- **Cross-platform**: features should work on macOS and Windows, and on Linux where possible. Put OS-specific code in `platform.rs` (or behind `cfg`) and say in the PR which platforms you tested.
- **UI text**: the app is available in English and Bahasa Indonesia. Wrap new text in `tr("…")` and add the Indonesian translation to the `ID` map in `src/lib/i18n.ts` (menus and notices from Rust: `src-tauri/src/i18n.rs`). If you can't write Indonesian, leave it out and mention it in the PR; the English text is the fallback.
- **Dependencies**: ask in an issue before adding a new dependency.
- **Commit messages**: a short summary in the imperative mood, e.g. `Fix scroll capture on Windows`.
- **Versions and changelog**: leave `CHANGELOG.md` and version numbers alone; they are updated at release time. Describe user-facing changes in the PR instead.

## Reporting bugs

Open an issue with:

- the KlikSnap version and your OS (and desktop environment on Linux)
- steps to reproduce
- what you expected and what happened
- relevant logs or screenshots, with anything private removed

## Security issues

Please don't report security problems in a public issue. Use **Report a vulnerability** on the repository's Security tab instead.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).

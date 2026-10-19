## What and why

<!-- What does this change, and why? Link the issue it fixes, e.g. "Fixes #12". -->

## How to test

<!-- Steps a reviewer can follow to see the change. Add a screenshot or short clip for UI changes. -->

## Tested on

- [ ] macOS
- [ ] Windows
- [ ] Linux (beta)

## Checklist

- [ ] `npm run check` and `npm run build` pass
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass in `src-tauri`
- [ ] New UI text uses `tr()` with an Indonesian translation in `src/lib/i18n.ts` (or `src-tauri/src/i18n.rs`)
- [ ] I've read [CONTRIBUTING.md](https://github.com/ashafizullah/kliksnap/blob/main/CONTRIBUTING.md)

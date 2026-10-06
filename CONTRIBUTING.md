# Contributing to Meownitor

Thanks for taking the time. Bug reports, ideas and pull requests are all welcome.

## Reporting a bug or asking for a feature

Open an [issue](https://github.com/kartatyi/meownitor/issues/new/choose) and pick the form that fits.
For a bug, the widget's log helps most: `~/.meownitor/widget.log` (`%USERPROFILE%\.meownitor` on
Windows), and for sessions that do not show up, `~/.meownitor/events.log`. They hold times and event and tool names,
nothing from your sessions' contents, but look them over before you attach them.

Security problems go to a private advisory instead, see [SECURITY.md](SECURITY.md).

## Building

You need [Rust](https://rustup.rs) (stable) and Node 20+, plus the MSVC build tools on Windows, the
Xcode command line tools on macOS, and these libraries on Debian or Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
```

```bash
cd app
npm install
cd src-tauri
cargo build      # target/debug/meownitor and meownitor-hook (.exe on Windows)
```

Run `target/debug/meownitor`. To try a mode without the mouse, set `MEOWNITOR_START` (`card`,
`settings`, `dock-l`, `dock-r`, `dock-r-open`) and `MEOWNITOR_MOOD` (`idle`, `work`, `ask`, `done`,
`sleep`, `tired`) before you start it.

## Before you open a pull request

CI runs the same three checks on every push, on Windows, macOS and Linux; run them first:

```bash
cd app/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
```

The tests change environment variables, hence one thread. Code for one system goes behind
`#[cfg(windows)]`, `#[cfg(target_os = "macos")]` or `#[cfg(all(unix, not(target_os = "macos")))]`
(Linux); clippy only sees the system it runs on, so CI is where the other two get checked. On Linux
and macOS, CI also starts the widget and keeps a screenshot as the run's `smoke-*` artifact.

If you change how the widget looks, attach a screenshot. The README pictures come from the app's own
pages on made-up data: `python docs/shoot.py` renders them again (Chrome or Edge and Pillow needed).

UI text lives in `app/src/i18n.js` (and `app/src-tauri/src/i18n.rs` for the tray menu); a new string
needs both English and Ukrainian.

## Commit messages

The history follows [Conventional Commits](https://www.conventionalcommits.org): a type, an optional
scope, and a subject in the imperative, up to 72 characters.

```
fix(limits): back off on 429 and 5xx, keep the last numbers

The body says what changed and why, wrapped at 72 characters.
```

Types used here: `feat`, `fix`, `refactor`, `build`, `ci`, `docs`, `chore`. Scopes: `sessions`,
`limits`, `rounds`, `settings`, `i18n`.

## Releasing

Maintainers only: set the version in `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml` and
`app/package.json`, add its section to [CHANGELOG.md](CHANGELOG.md), then push a `vX.Y.Z` tag. The
release workflow does the rest.

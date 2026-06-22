# UVie Rust Win

Telex-only Vietnamese input method for Windows, built as a small tray app in Rust.

UVie Rust Win intercepts keyboard input, converts Telex sequences to precomposed Unicode Vietnamese text, and uses app-specific replacement strategies for browsers, editors, terminals, and password fields.

## Install

1. Open the latest GitHub release:
   <https://github.com/soncms/uvie-rs-win/releases/latest>
2. Download either:
   - `uvie-rs-win.exe` for the standalone executable.
   - `uvie-rs-win-windows-x86_64.zip` for the executable plus README.
3. Put `uvie-rs-win.exe` in a writable folder.
4. Run `uvie-rs-win.exe`.

The app creates `uvie-rs-win.json` beside the executable on first run.

## Usage

- Left-click the tray icon to toggle Vietnamese/English mode.
- Right-click the tray icon to open the menu.
- Press and release `Ctrl+Shift` by itself to toggle Vietnamese/English mode.
- Normal shortcuts such as `Ctrl+Shift+P` pass through to the active app.
- `Chạy cùng Windows` toggles startup registration.
- `Chạy bằng Admin` relaunches/registers the app with admin rights when needed.

Supported scope:

- Telex input only.
- Unicode precomposed Vietnamese output.
- No VNI, Macro, TCVN3, VNI encoding, or CP1258 conversion.

## Config

Default config:

```json
{
  "enabled": true,
  "runAtStartup": false,
  "runAsAdmin": false,
  "hotkey": "Ctrl+Shift",
  "quickTelex": false,
  "debugLog": false,
  "rules": [],
  "excludedApps": []
}
```

Useful fields:

- `enabled`: starts the app in Vietnamese mode when `true`.
- `runAtStartup`: registers the app to run with Windows.
- `runAsAdmin`: relaunches through the app's admin helper.
- `hotkey`: currently supports `Ctrl+Shift`.
- `quickTelex`: off by default for stability.
- `debugLog`: writes replacement/context logs for QA.
- `rules`: optional app/profile overrides.
- `excludedApps`: process names that should pass through unchanged.

Example rule config is available in [uvie-rs-win.sample.json](uvie-rs-win.sample.json).

## Build From Source

Requirements:

- Windows x86_64
- Rust stable toolchain
- PowerShell

Build:

```powershell
cargo test --locked
cargo build --release --locked
```

Run the local build:

```powershell
.\target\release\uvie-rs-win.exe
```

Release QA gate:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
```

If the release executable is locked because the tray app is running:

```powershell
Stop-Process -Name uvie-rs-win -Force
cargo build --release --locked
```

Do not use an alternate target directory for release builds.

## Create A GitHub Release

The release workflow runs on `v*` tags. It tests, builds, packages, and publishes:

- `uvie-rs-win.exe`
- `uvie-rs-win-windows-x86_64.zip`

Create a release:

```powershell
$tag = "v0.1.2"
git tag $tag
git push origin $tag
```

Use the next version tag for later releases.

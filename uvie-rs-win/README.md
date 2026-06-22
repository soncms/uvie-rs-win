# UVie Rust Win

Telex-only Windows tray app built in Rust. This is the release path; the old C++ prototype has been removed.

## Build

```powershell
cargo test
cargo build --release
```

Run:

```powershell
.\target\release\uvie-rs-win.exe
```

## GitHub release

Push a version tag to build and publish a Windows `.exe`:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## Behavior

- Left-click tray icon toggles Vietnamese/English.
- Right-click tray icon opens menu.
- Press and release `Ctrl+Shift` by itself to toggle Vietnamese/English.
- Combos such as `Ctrl+Shift+P` pass through to the active app.
- `Chạy cùng Windows` toggles startup.
- `Chạy bằng Admin` toggles normal/admin relaunch.

## Config

The app writes `uvie-rs-win.json` beside the executable:

```json
{
  "enabled": true,
  "runAtStartup": false,
  "runAsAdmin": false,
  "hotkey": "Ctrl+Shift",
  "quickTelex": false
}
```

`quickTelex` is off by default for stability.

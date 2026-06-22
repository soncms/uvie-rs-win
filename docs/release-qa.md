# UVie Rust Win Release QA

Scope: Telex-only, Unicode precomposed output. No Macro, no VNI, no legacy encodings.

## Automated Gate

Run from the repository root:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

If `target\release\uvie-rs-win.exe` is locked by the running tray app:

```powershell
Stop-Process -Name uvie-rs-win -Force
cargo build --release
```

Do not build to an alternate target directory for release.

## Manual Matrix

Notepad:
- `tieengs`, `nguwfoi`, `dduowcj`, `tooi laf`
- `go ` then Backspace then `o` -> `gô`
- `google`, `workflow`, `account`, `window`

Browsers:
- Chrome, Edge, Thorium address bar: no duplicated `aâ`, no autocomplete duplication.
- Page input and textarea: normal Telex replacement.
- Google Docs/Notion: verify slow text-field rule behavior if debug log identifies built-in rule.

Terminal:
- Windows Terminal, PowerShell, cmd.
- Verify slow replacement does not reorder or swallow keys.

Safety:
- Browser password field pass-through, no raw/visible text logged.
- Excluded apps in `excludedApps` pass-through and reset session.

## Debug Log Fields

When `debugLog` is enabled, replacement lines include:

```text
exe title class focusType focusName focusClass automationId behavior strategy strategyChain reason confidence ruleName raw visible edit
```

Password pass-through must not log typed content.

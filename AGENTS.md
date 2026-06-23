# AGENTS.md

## Structure

Single Go module (`proxy-manager`), single `main.go` — no packages, no tests, no CI.

## Build & Run

```bash
go build .
./proxy-manager
```

Requires Go 1.26.2+, Fyne system dependencies, and `pkexec` (PolicyKit).

## Key quirks

- **Privilege escalation**: writing to `/etc/environment` re-executes itself via `pkexec` using the `-apply` subcommand. The binary passes all config as flags (`-user`, `-pass`, `-url`, `-port`, `-noproxy`, `-file`, `-enable`).
- **Hardcoded bashrc path**: `Bashrc = "/home/julio/.bashrc"` (not `$HOME` or `~`).
- **Embedded icon**: `picture.png` must exist at build time (`//go:embed picture.png`).
- **No tests** — nothing to run.
- **Gitignore**: both binary `proxy-manager` and `proxy-manager.desktop` are gitignored.

## Fyne system dependencies

See https://docs.fyne.io/started/#prerequisites — typically `libgl1-mesa-dev xorg-dev` on Debian/Ubuntu.

# dd_dotstore

TUI to manage Linux dotfiles: pick a project folder, assign destinations, symlink or copy in bulk, persist the map.

![Main window](docs/images/01-main.png)

**Illustrated tutorial (install, first run, keys, screenshots):** [ldnddev.github.io/dd_dotstore](https://ldnddev.github.io/dd_dotstore/)

## Install

The installer detects Linux `x86_64` / `aarch64` and installs the musl package from GitHub Releases plus the theme.

The repo is public:

```bash
curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_dotstore/master/install.sh | bash

dd_dotstore --help
```

`GITHUB_TOKEN` is optional and only needed if you hit GitHub API rate limits.

From a source checkout: `./install.sh`. That installs the binary, the theme, and a desktop launcher. Uninstall: `./install.sh --uninstall` (removes those installed files). Full options and PATH notes are in the [tutorial](https://ldnddev.github.io/dd_dotstore/#install).

## Run

```bash
cargo run
cargo run -- --root /path/to/dotfiles
cargo run -- --help
cargo test
```

Requires Rust 1.85+ (edition 2024) and a Unix-like OS (symlinks). Config is `.dd_dotstore.json` in the project root.

## What you get

- Tree of the project, multi-select, LINK or COPY per item
- Smart dest defaults (XDG / `$HOME` / `.linked` fallback)
- Plan dialog, overwrite warnings, undo (last 10, this session)
- Groups, reverse-import (`A`), doctor (`D`)
- Theme editor (`F2`), ignore editor, import/export
- Keyboard and mouse; help is `F1`

## License

MIT

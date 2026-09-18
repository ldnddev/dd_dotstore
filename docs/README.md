# Docs

The illustrated tutorial is a GitHub Pages site:

**https://ldnddev.github.io/dd_dotstore/**

GitHub does not execute HTML in the file browser, so do not send users to `docs/index.html` on github.com. The Pages workflow publishes `index.html` plus the PNGs to that URL.

`.github/workflows/pages.yml` deploys on pushes to `master` that touch `docs/index.html` or `docs/images/`. It copies only the tutorial HTML and PNGs (not `capture.sh` or this file). `docs/.nojekyll` keeps GitHub from running Jekyll over the static page.

Local preview:

```bash
# from the repo root
xdg-open docs/index.html
```

## Updating tutorial screenshots

PNGs under `docs/images/` are generated from the live TUI. Do not edit them in an image editor — change the capture scene and regenerate.

```bash
./docs/capture.sh
```

### What the script does

1. `cargo run --example tutorial_shots --release`  
   Builds a throwaway fixture (`/tmp/dd-dots` project, `/tmp/dd-home` as `$HOME` / XDG) so dest paths stay short and adopt/doctor never walk your real home. Drives the app with the same key handler the TUI uses, draws each scene to a ratatui `TestBackend`, and writes HTML frames to `docs/images/_frames/` (gitignored).
2. `node docs/rasterize.mjs`  
   Opens each frame in Chromium via Playwright and screenshots the `.shot` card to `docs/images/<id>.png` at 2×.
3. Greps `docs/index.html` for `images/*.png` and exits non-zero if a referenced file is missing.

Commit the new PNGs with the tutorial copy change.

### Requirements

- Rust toolchain (same as the crate: 1.85+)
- Node, with the `playwright` package resolvable
- Chromium at `/usr/bin/chromium` (or set `TUTORIAL_CHROMIUM`)

On this machine Playwright is typically:

```text
~/.local/share/mise/installs/npm-playwright/latest/node_modules
```

Override with `PLAYWRIGHT_NODE_MODULES` if yours lives elsewhere. `docs/capture.sh` also tries `require.resolve("playwright")`.

### Shot catalog

| File | Scene in `examples/tutorial_shots.rs` |
|------|----------------------------------------|
| `01-main.png` | Expanded tree, mixed ✓ / ○ / ✗, dest panel |
| `02-help.png` | `F1` |
| `03-filter.png` | `/` then `nvim` |
| `04-dest-browser.png` | Highlight `.bashrc`, `e` |
| `05-plan.png` | Highlight `.config/nvim`, `s` |
| `06-overwrite.png` | Highlight `.gitconfig`, `s` then `Y` |
| `07-theme.png` | `F2` |
| `08-ignore.png` | `I` |
| `09-doctor.png` | `D` |
| `10-credits.png` | `F3` |
| `11-group.png` | Highlight `.zshrc`, `t` |
| `12-adopt.png` | `A` |
| `13-selected.png` | `T` on the `shell` group (toast) |
| `14-copy-mode.png` | `m` on `scripts/bootstrap.sh` |

The header quote is pinned to `Don't Fear the . (Dot) - Tame It.` so screenshots do not flicker between taglines.

### Add or change a shot

1. Edit `examples/tutorial_shots.rs`: after the app is in the state you want, call `capture(&mut app, &frames_dir, &mut shots, "15-name", "caption")`.
2. Add a row to the table above.
3. Use the PNG from `docs/index.html` (`<img src="images/15-name.png" alt="...">`). Give `alt` a description of what the reader should learn from the frame, not “screenshot”.
4. Run `./docs/capture.sh`.
5. Open `docs/index.html` and confirm the new figure.

To change the fixture (files, dests, broken/orphan links), edit `rebuild_fixture` in the example. Keep dests under `/tmp/dd-home` so they fit the source column.

### Do not

- Point `$HOME` at a real user directory while capturing (adopt/doctor will leak private paths into the PNGs).
- Check in `docs/images/_frames/` — those HTML files are intermediates.
- Hand-patch a PNG to hide a UI bug; fix the TUI or the scene instead.

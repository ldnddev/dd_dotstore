# Header and Footer Guide (Consistent UI Shell for ldnddev TUIs)

This document specifies the **header** and **footer** (status bar) used across ldnddev TUI apps (starting with dd_dotstore). The goal is to give every app the same professional, clean, discoverable "shell" look and feel while leaving the main content area flexible.

Use this alongside:
- `THEME_STRUCTURE_STANDARD.md` (for colors and styles)
- `SOURCE_PANEL_GUIDE.md` (for the main content area navigation)

## Overall Layout Structure

In the main `draw()` function (see `src/ui.rs`):

```rust
let outer = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),   // HEADER - always 3 lines
        Constraint::Min(0),      // MAIN CONTENT (source + right panel, etc.)
        Constraint::Length(1),   // FOOTER - now 1 line after declutter
    ])
    .split(f.area());

draw_header(f, state, outer[0]);
draw_main_content... (outer[1])
draw_status_bar(f, state, outer[2]);
```

- Header is **fixed 3 lines high**.
- Footer is **fixed 1 line high** (reduced from 2 for declutter; the old theme status line was moved out).
- The middle area gets all remaining height.
- The entire app starts with a full-screen `Block` using `app_shell` style (base_background + text_primary) for consistent outer shell.

**Recommendation for reuse**: Hard-code these heights in every app. Do **not** make them dynamic unless you have a very good reason (it breaks the consistent feel).

## Header (draw_header)

### Visual Structure
- A `Paragraph` inside a `Block` with:
  - `.title("PROJECT_NAME")`  (e.g. "dd_dotstore", "dd_ftp")
  - `.borders(Borders::ALL)`
  - `.border_style(theme.active_border)`
  - `.style(theme.app_shell)`  (primary text on base_background)
- **Content**: `state.header_copy` (a single line of witty text)
- Height is always 3 lines (the block + 1 line of content + borders).

### Content (the tagline)
- Randomized at startup in `App::new...` (see `src/app.rs`):
  ```rust
  state.header_copy = random_header_copy().to_string();
  ```
- `random_header_copy()` picks from a small const array of taglines using a seed based on current time (nanos) XOR process ID.
- Examples from dd_dotstore (feel free to have app-specific ones, but keep the same spirit and randomization mechanism):
  - "Don't Fear the . (Dot) - Tame It."
  - ". (Dot) file Domination done right."
  - ". (Dot) file management fatigue is real. Or used to be."
  - etc.

**For consistency across apps**:
- Use the same randomization logic (copy the function or share it later).
- Keep taglines short (one line).
- Make them fun, thematic, and a little irreverent (matches the "ldnddev" personality).
- Store them as `&'static str` slices.

### Theming
- Uses `app_shell` (base_background + text_primary) for the whole block + content.
- Title uses `active_border` color.
- Never hardcode colors here — always pull from the loaded theme (see THEME_STRUCTURE_STANDARD.md).

### Behavior
- Purely decorative + branding.
- No interaction (mouse/keyboard on header does nothing special).
- Updates only on app restart (randomized once).

## Footer / Status Bar (draw_status_bar)

### Visual Structure (Current Design)
- A `Paragraph` with **no border** (clean minimal look).
- `.style(theme.app_shell)` (same as header for shell consistency).
- Currently **exactly one line** of content (the key hints).
- Height fixed to `Length(1)` in the outer layout.

**History note**: Previously 2 lines (theme status + keys). The first line was removed for declutter on small screens. Theme health info lives in:
- F2 Credits modal
- Startup (initial footer or toast)
- Warning states

### Content: Adaptive Key Hints
The single line is a width-adaptive string of common keybindings:

```rust
let keys = if area.width < 75 {
    "F1:Help  q:Quit  j/k:Nav  Spc:Sel  s:Apply  x:Rem  /:Filter"
} else if area.width < 110 {
    "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   Q: Exit"
} else {
    "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   Q: Exit   (mouse: click/scroll/drag)"
};
```

**Rules for consistency**:
- Always start with `F1:Help` (or equivalent) — it's the escape hatch for full controls.
- Use very short abbreviations on narrow terminals.
- Include the most-used actions for the app.
- On wide terminals, add a short parenthetical for advanced features (mouse in this case).
- Never let the line get so long that it wraps or looks cramped (the adaptive logic prevents this).
- Keep the same visual style (spaces for separation, no heavy punctuation).

**For other apps (e.g. dd_ftp)**:
- Replace the actions with your equivalents (e.g. "u:Upload  d:Download  ...").
- Keep the structure: F1 first, then navigation, then primary actions, then quit.
- Document your specific key set in the F1 Help modal so the footer is just a reminder.

### Theming
- Whole line uses `app_shell`.
- No borders.
- If you ever re-add a left-side status (e.g. "Theme OK..." or connection status), use appropriate semantic colors (info, warning, etc.) but keep it minimal.

### Behavior
- Purely informational / discoverability.
- Updates every frame (the width check is cheap).
- No mouse/keyboard handling on the footer itself (clicking it does nothing).
- The adaptive nature is part of the "polish" — it feels thoughtful on any terminal size.

## Recommended Shell Theme Tokens (from THEME_STRUCTURE_STANDARD.md)

These must be consistent:

- `app_shell`: base_background + text_primary  (outer shell for header + footer)
- `active_border`: for the header's title border
- `body_background` / etc. are for the main content area only

See the full theme standard for the complete list and mapping rules. Do not invent new colors for the shell.

## Implementation Pattern (Copy This)

1. In `AppState`:
   ```rust
   pub header_copy: String,
   // (footer has no extra state beyond what the keys need)
   ```

2. In `App::new...`:
   ```rust
   state.header_copy = random_header_copy().to_string();
   ```

3. `random_header_copy()` function (copy the one from app.rs, customize the COPIES array).

4. In `draw()`:
   - Fixed outer vertical layout with Length(3) + Min(0) + Length(1)
   - Call `draw_header` and `draw_status_bar` exactly as shown.

5. Implement `draw_header` and `draw_status_bar` exactly as in the current `ui.rs` (or adapt the adaptive logic).

6. In the F1 Help modal, show the **full** key list + mouse details so the footer can stay short.

7. In Credits (F2), show theme source/status (this replaces the old persistent footer line).

## Anti-Patterns to Avoid

- Making header/footer height dynamic based on content or terminal size (breaks consistency).
- Putting long status text in the footer (theme health, connection status, etc.) — move it to modals or a dedicated status line only when critical.
- Hard-coding key hints without the width-adaptive logic.
- Using different border styles or background colors for header vs. the rest of the shell.
- Forgetting to use `theme.app_shell` and `theme.active_border`.

## Future Sharing Ideas

Once several apps exist, consider extracting:
- A small `ldnddev_tui_shell` crate with `draw_header`, `draw_status_bar`, `random_header_copy`, and the layout constants.
- Or at minimum keep these two MD files (this one + SOURCE_PANEL_GUIDE.md + THEME_STRUCTURE_STANDARD.md) as the "visual contract."

## Files to Study in dd_dotstore

- `src/ui.rs`: `draw_header`, `draw_status_bar`, the outer layout in `draw()`
- `src/app.rs`: `random_header_copy` + where `header_copy` is set
- `src/state.rs`: the `header_copy` field + theme `app_shell` / `active_border`
- Current `dd_dotstore_theme.yml` for example values
- `THEME_STRUCTURE_STANDARD.md`
- F1 Help modal text (for full key reference)

Follow this and your dd_ftp (and future apps) will feel like part of the same family — same clean header branding, same minimal adaptive footer, same shell colors — while the content areas can be completely different.

If you want a combined "LDNDDEV TUI VISUAL STANDARD.md" that merges theme + header/footer + source panel, let me know and I'll create it.
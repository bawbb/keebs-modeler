# Presets

**Where:** `presets/<name>.preset` at the workspace root, found via `CARGO_MANIFEST_DIR` so the app, `render`, `survey`, and `pedal` agree whatever the working directory. Kept in the repo (not a user config folder) so good sounds can be committed and diffed alongside the code that makes them. Revisit for the hardware port.

**Format:** plain text, one `key = value` per parameter, using the same keys as the command line (`render help`); `#` comments. Chosen over JSON/TOML to avoid a serde dependency and to keep diffs one line per change. Values are written with `f32` `Display`, which round-trips exactly (tested).

**Tolerance:** parsing starts from defaults, so a preset saved before a parameter existed loads with that parameter at its default. Unknown keys (renamed/removed parameters) and out-of-range values are skipped/clamped with a warning, shown in the app (hover) and on stderr for the tools. **Renaming a parameter's `key` breaks it in old presets** — prefer keeping keys stable.

**Code:** text ↔ `ParamValues` lives in `core/src/preset.rs` (no I/O); files in `app/src/presets.rs`. Saves write a temp file then rename, so a failed save can't truncate a preset.

**App:** `Default` is a built-in entry at the top of the list (all parameter defaults, not a file); the name is reserved, so it can't be saved over or deleted. It replaced the old "Reset all to defaults" button, and the app starts with it loaded (or `--preset <name>`). A preset is "modified" when the live values differ from the values last loaded/saved under that name; Revert restores them. Computer-key note presses are ignored while a text box has focus (typing a name otherwise played notes); releases still pass so nothing sticks.

## Using pickers

Helix has a variety of pickers, which are interactive windows used to select various kinds of items. These include a file picker, global search picker, and more. Most pickers are accessed via keybindings in [space mode](./keymap.md#space-mode). Pickers have their own [keymap](./keymap.md#picker) for navigation.

### Filtering Picker Results

Most pickers perform fuzzy matching using [fzf syntax](https://github.com/junegunn/fzf?tab=readme-ov-file#search-syntax). Two exceptions are the global search picker, which uses regex, and the workspace symbol picker, which passes search terms to the language server. Note that OR operations (`|`) are not currently supported.

If a picker shows multiple columns, you may apply the filter to a specific column by prefixing the column name with `%`. Column names can be shortened to any prefix, so `%p`, `%pa` or `%pat` all mean the same as `%path`. For example, a query of `helix %p .toml !lang` in the global search picker searches for the term "helix" within files with paths ending in ".toml" but not including "lang".

You can insert the contents of a [register](./registers.md) using `Ctrl-r` followed by a register name. For example, one could insert the currently selected text using `Ctrl-r`-`.`, or the directory of the current file using `Ctrl-r`-`%` followed by `Ctrl-w` to remove the last path section. The global search picker will use the contents of the [search register](./registers.md#default-registers) if you press `Enter` without typing a filter. For example, pressing `*`-`Space-/`-`Enter` will start a global search for the currently selected text.

### File picker and file explorer

`Space-f` opens the fuzzy file picker, rooted at the workspace; `Space-F` opens it at the current working directory. When the terminal is wide enough, the picker shows a syntax-highlighted preview of the selected file by default. Press `Ctrl-t` to toggle the preview.

`Space-e` toggles the file explorer sidebar, rooted at the workspace. `Space-.` opens it at the current buffer's directory. Its ignore behaviour and layout are configured in the [`[editor.file-explorer]`](./editor.md#editorfile-explorer-section) section. The tree refreshes automatically when files under the currently expanded directories change; press `r` to force a full refresh.

When the workspace is a git repository, changed files are marked with `M` (modified), `?` (untracked), `D` (deleted), `R` (renamed) or `!` (conflict). Directory markers aggregate the status of their descendants. The markers use the theme's `diff.plus`, `diff.delta`, `diff.minus`, `diff.delta.moved` and `diff.delta.conflict` styles.

When the sidebar is focused (`Space-e` opens it focused), the following keys are available:

| Key | Action |
|--|--|
| `j` / `k` | Move selection down / up |
| `h` / `l` | Collapse / expand the selected directory |
| `Enter` | Toggle a directory or open a file |
| `v` / `s` | Open the selected file in a vertical / horizontal split |
| `g` / `G` | Jump to the first / last entry |
| `r` | Refresh the directory cache |
| `R` | Reveal the active buffer in the tree |
| `a` | Create a new file in the selected directory |
| `A` | Create a new directory in the selected directory |
| `m` | Rename or move the selected file/directory |
| `d` | Delete the selected file/directory (asks for confirmation) |
| `q` | Hide the sidebar |
| `Esc` | Return focus to the editor, keeping the sidebar visible |
| `<space>e` | Hide the sidebar |

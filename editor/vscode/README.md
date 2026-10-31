# Pillar for VS Code

Build or install `plrc`, then run `npm ci --ignore-scripts` in this directory.
Open the repository with `code --extensionDevelopmentPath=/absolute/path/to/pillar/editor/vscode`
and open a `.plr` file. Set `pillar.compilerPath` if `plrc` is not on PATH.

The extension provides diagnostics, hover, definitions, rename, semantic highlighting,
formatting and completion through `plrc lsp`. Command Palette actions insert text
downward, insert/remove/move a column, and show a live read-only horizontal preview.
Downward insertion also uses Ctrl+Alt+Enter (Cmd+Alt+Enter on macOS).

Rename can change the height of a column and is applied as a single document edit.
The server renames references reachable from the active source; it does not index
unopened importers across an entire workspace. Untrusted workspaces do not activate
the extension. To create a local VSIX, run `npx @vscode/vsce package` after installing
this directory's dependencies. No extension has been published to a marketplace.

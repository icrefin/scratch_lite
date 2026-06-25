# ScratchLite

A simplified, editor-only version of [Scratch](https://github.com/erictli/scratch) — a minimalist markdown note-taking app.

ScratchLite strips Scratch down to its core: a distraction-free WYSIWYG markdown editor for individual `.md` files. No notes folder management, no sidebar, no git integration, no full-text search — just you and your markdown.

## Differences from Scratch

ScratchLite removes:
- **Notes folder** — No need to pick a notes folder; open individual `.md` files directly
- **Sidebar** — No note list, folder tree, or pinning
- **Git integration** — No automatic version control
- **Full-text search** — No Tantivy search index
- **Welcome screen** — Starts with a blank unnamed file instead

Everything else — WYSIWYG editing, export, focus mode, themes, keyboard shortcuts, command palette, vim mode — remains intact.

## Features

- **WYSIWYG markdown editing** — Rich text that saves as plain `.md`
- **Markdown source mode** — Toggle to raw markdown (`Cmd+Shift+M`)
- **Syntax highlighting** — 20+ languages
- **Mermaid diagrams** — Flowcharts, sequence diagrams, and more
- **KaTeX math** — Block and inline math rendering
- **Focus mode** — Distraction-free writing (`Cmd+Shift+Enter`)
- **Vim mode** — Modal editing with Ex command bar (`:s`, `:nohl`, `:w`)
- **Command palette** — `Cmd+P` for quick everything access
- **Export** — Copy as Markdown, Plain Text, HTML; Print as PDF
- **Customizable** — Theme, typography, page width, RTL text

## Installation (macOS)

Download the `.dmg` from the [releases page](https://github.com/icrefin/scratch_lite/releases), mount it, and drag ScratchLite to your Applications folder.

If macOS blocks the app because it is not notarized, run this in Terminal:

```bash
xattr -dr com.apple.quarantine /Applications/ScratchLite.app
```

This removes the quarantine flag set by macOS on files downloaded from the internet. You only need to do this once after the first download.

## Credits

ScratchLite is a fork of [Scratch](https://github.com/erictli/scratch) by [Eric Li](https://ericli.io). All credit for the original app and its design goes to Eric Li and the Scratch contributors. Please support the original project.

## License

MIT. Based on [Scratch](https://github.com/erictli/scratch) © Eric Li.

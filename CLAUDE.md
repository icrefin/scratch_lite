# ScratchLite - Development Guide

## Project Overview

ScratchLite is a simplified, editor-only markdown scratchpad for macOS, Windows, and Linux, built with Tauri v2 (Rust backend) + React/TypeScript/Tailwind (frontend) + TipTap (WYSIWYG editor). It is based on [Scratch](https://github.com/erictli/scratch) by Eric Li.

## Commands

```bash
npm run dev          # Start Vite dev server only
npm run build        # Build frontend (tsc + vite)
npm run tauri dev    # Run full app in development mode
npm run tauri build  # Build production app
```

## Key Patterns

- All backend operations go through Tauri commands in `src-tauri/src/lib.rs`. Frontend calls them via `invoke()` from `@tauri-apps/api/core`.
- Settings live at `{APP_DATA}/settings.json`.
- Tauri v2 permissions go in `src-tauri/capabilities/default.json`.

## Coding Conventions

- Clean, minimal code with low technical debt
- Proper React patterns (contexts, hooks, memoization)
- Type-safe with TypeScript throughout
- No commented-out code or TODOs in production code
- Use `React.memo` for expensive list-item components
- Use `useCallback`/`useMemo` for performance-critical paths
- Debounce user-triggered operations (auto-save 300ms, file watcher 500ms)
- All operations should be non-blocking (async)
- Error handling with user-friendly messages

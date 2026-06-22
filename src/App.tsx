import { useState, useEffect, useCallback, useRef } from "react";
import { toast } from "sonner";
import { ThemeProvider, useTheme } from "./context/ThemeContext";
import { TooltipProvider, Toaster } from "./components/ui";
import { Editor, type PreviewModeData } from "./components/editor/Editor";
import type { Editor as TiptapEditor } from "@tiptap/react";
import { CommandPalette } from "./components/command-palette/CommandPalette";
import { SettingsPage } from "./components/settings";
import { AiEditModal } from "./components/ai/AiEditModal";
import { AiResponseToast } from "./components/ai/AiResponseToast";
import { KeyboardShortcutsModal } from "./components/shortcuts/KeyboardShortcutsModal";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as filesService from "./services/files";
import * as aiService from "./services/ai";
import type { AiProvider } from "./services/ai";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ViewState = "editor" | "settings";

function AppContent() {
  const { interfaceZoom, setInterfaceZoom } = useTheme();
  const interfaceZoomRef = useRef(interfaceZoom);
  interfaceZoomRef.current = interfaceZoom;

  const [view, setView] = useState<ViewState>("editor");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [aiModalOpen, setAiModalOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [aiEditing, setAiEditing] = useState(false);
  const [focusMode, setFocusMode] = useState(false);
  const [aiProvider, setAiProvider] = useState<AiProvider>("claude");
  const editorRef = useRef<TiptapEditor | null>(null);

  // Preview mode data (for the editor)
  // Start with an unnamed empty file in memory
  const [filePath, setFilePath] = useState<string | null>(null);
  const [content, setContent] = useState<string | null>("");
  const [title, setTitle] = useState("Untitled");
  const [modified, setModified] = useState(() => Math.floor(Date.now() / 1000));
  const [hasExternalChanges, setHasExternalChanges] = useState(false);
  const [reloadVersion, setReloadVersion] = useState(0);
  const recentlySavedRef = useRef(false);

  const loadFile = useCallback(async (path: string) => {
    try {
      const result = await filesService.readFileDirect(path);
      setFilePath(path);
      setContent(result.content);
      setTitle(result.title);
      setModified(result.modified);
      setHasExternalChanges(false);
      // Add to recent files
      await invoke("add_recent_file", { path });
    } catch (error) {
      console.error("Failed to load file:", error);
      toast.error(`Failed to load file: ${error}`);
    }
  }, []);

  const loadFileRef = useRef(loadFile);
  loadFileRef.current = loadFile;

  // Listen for file-open events from backend (CLI args, Open With, drag-drop)
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<string>("open-file", (event) => {
      if (cancelled) return;
      loadFile(event.payload);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [loadFile]);

  // Listen for menu events from native menu bar
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<string>("menu-event", (event) => {
      if (cancelled) return;
      switch (event.payload) {
        case "new_file": {
          const dialog = async () => {
            try {
              const { save: saveFile } = await import("@tauri-apps/plugin-dialog");
              const path = await saveFile({
                filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
                defaultPath: "untitled.md",
              });
              if (path) {
                await invoke("write_file", { path, contents: new TextEncoder().encode("# Untitled\n\n") });
                loadFileRef.current(path);
              }
            } catch (error) {
              console.error("Failed to create new file:", error);
              toast.error("Failed to create new file");
            }
          };
          dialog();
          break;
        }
        case "open":
          openFileDialogRef.current();
          break;
        case "save": {
          const editor = editorRef.current;
          if (editor) {
            const manager = editor.storage.markdown?.manager;
            const markdown = manager
              ? manager.serialize(editor.getJSON())
              : editor.getText();
            saveRef.current(markdown);
          }
          break;
        }
        case "save_as": {
          const editor = editorRef.current;
          if (editor) {
            const manager = editor.storage.markdown?.manager;
            const markdown = manager
              ? manager.serialize(editor.getJSON())
              : editor.getText();
            const dialog = async () => {
              const fp = await import("@tauri-apps/plugin-dialog").then((m) =>
                m.save({
                  filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
                  defaultPath: "untitled.md",
                })
              );
              if (!fp) return;
              try {
                const result = await filesService.saveFileDirect(fp, markdown);
                recentlySavedRef.current = true;
                setFilePath(fp);
                setModified(result.modified);
                setTitle(result.title);
                setHasExternalChanges(false);
              } catch (error) {
                console.error("Failed to save file:", error);
                toast.error(`Failed to save: ${error}`);
              }
            };
            dialog();
          }
          break;
        }
        case "print":
          window.dispatchEvent(new CustomEvent("print-note"));
          break;
        case "settings":
          toggleSettingsRef.current();
          break;
        case "zoom_in": {
          const newZoom = Math.round(Math.min(interfaceZoomRef.current + 0.05, 1.5) * 20) / 20;
          setInterfaceZoom(newZoom);
          break;
        }
        case "zoom_out": {
          const newZoom = Math.round(Math.max(interfaceZoomRef.current - 0.05, 0.7) * 20) / 20;
          setInterfaceZoom(newZoom);
          break;
        }
        case "zoom_reset":
          setInterfaceZoom(1.0);
          break;
        case "toggle_focus":
          toggleFocusModeRef.current();
          break;
        case "toggle_source":
          window.dispatchEvent(new CustomEvent("toggle-source-mode"));
          break;
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Check URL params for file on mount (from CLI args)
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const file = params.get("file");
    if (file) {
      loadFile(decodeURIComponent(file));
    }
  }, [loadFile]);

  // Listen for window focus to detect external changes
  useEffect(() => {
    const handleFocus = async () => {
      if (recentlySavedRef.current || !filePath) {
        recentlySavedRef.current = false;
        return;
      }
      try {
        const result = await filesService.readFileDirect(filePath);
        if (result.modified !== modified && content !== null) {
          setHasExternalChanges(true);
        }
      } catch {
        // File may have been deleted
      }
    };

    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, [filePath, modified, content]);

  const save = useCallback(
    async (newContent: string) => {
      let targetPath = filePath;

      // Unnamed file — trigger Save As dialog
      if (!targetPath) {
        try {
          const { save: saveFile } = await import("@tauri-apps/plugin-dialog");
          const selected = await saveFile({
            filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
            defaultPath: "untitled.md",
          });
          if (!selected) return;
          targetPath = selected;
        } catch (error) {
          console.error("Failed to save:", error);
          toast.error(`Failed to save: ${error}`);
          return;
        }
      }

      try {
        const result = await filesService.saveFileDirect(targetPath, newContent);
        recentlySavedRef.current = true;
        setFilePath(targetPath);
        setModified(result.modified);
        setTitle(result.title);
        setHasExternalChanges(false);
      } catch (error) {
        console.error("Failed to save file:", error);
        toast.error(`Failed to save: ${error}`);
      }
    },
    [filePath],
  );

  const saveRef = useRef(save);
  saveRef.current = save;

  const reload = useCallback(async () => {
    if (!filePath) return;
    try {
      const result = await filesService.readFileDirect(filePath);
      setContent(result.content);
      setTitle(result.title);
      setModified(result.modified);
      setHasExternalChanges(false);
      setReloadVersion((v) => v + 1);
    } catch (error) {
      console.error("Failed to reload file:", error);
      toast.error(`Failed to reload: ${error}`);
    }
  }, [filePath]);

  const openFileDialog = useCallback(async () => {
    try {
      const selected = await invoke<string | null>("open_file_dialog");
      if (selected) {
        loadFile(selected);
      }
    } catch (error) {
      console.error("Failed to open file:", error);
      toast.error("Failed to open file");
    }
  }, [loadFile]);

  const openFileDialogRef = useRef(openFileDialog);
  openFileDialogRef.current = openFileDialog;

  const toggleSettings = useCallback(() => {
    setView((prev) => (prev === "settings" ? "editor" : "settings"));
  }, []);

  const toggleSettingsRef = useRef(toggleSettings);
  toggleSettingsRef.current = toggleSettings;

  const closeSettings = useCallback(() => {
    setView("editor");
  }, []);

  const toggleFocusMode = useCallback(() => {
    setFocusMode((prev) => !prev);
  }, []);

  const toggleFocusModeRef = useRef(toggleFocusMode);
  toggleFocusModeRef.current = toggleFocusMode;

  // Back to command palette from AI modal
  const handleBackToPalette = useCallback(() => {
    setAiModalOpen(false);
    setPaletteOpen(true);
  }, []);

  // AI Edit handler
  const handleAiEdit = useCallback(
    async (prompt: string, ollamaModel?: string) => {
      if (!filePath) {
        toast.error("No file open");
        return;
      }

      setAiEditing(true);

      try {
        let result: aiService.AiExecutionResult;
        if (aiProvider === "codex") {
          result = await aiService.executeCodexEdit(filePath, prompt);
        } else if (aiProvider === "opencode") {
          result = await aiService.executeOpenCodeEdit(filePath, prompt);
        } else if (aiProvider === "ollama") {
          result = await aiService.executeOllamaEdit(
            filePath,
            prompt,
            ollamaModel || "qwen3:8b",
          );
        } else {
          result = await aiService.executeClaudeEdit(filePath, prompt);
        }

        // Reload file from disk
        await reload();

        // Show results
        if (result.success) {
          setAiModalOpen(false);
          toast(
            <AiResponseToast output={result.output} provider={aiProvider} />,
            {
              duration: Infinity,
              closeButton: true,
              className: "!min-w-[450px] !max-w-[600px]",
            },
          );
        } else {
          toast.error(
            <div className="space-y-1">
              <div className="font-medium">AI Edit Failed</div>
              <div className="text-xs">{result.error || "Unknown error"}</div>
            </div>,
            { duration: Infinity, closeButton: true },
          );
        }
      } catch (error) {
        console.error("[AI] Error:", error);
        toast.error(
          `Error: ${error instanceof Error ? error.message : "Unknown error"}`,
        );
      } finally {
        setAiEditing(false);
      }
    },
    [aiProvider, filePath, reload],
  );

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isInEditor = !!target.closest(".ProseMirror");

      // Cmd+, - Toggle settings (always works)
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        toggleSettings();
        return;
      }

      // Cmd+= or Cmd++ - Zoom in
      if ((e.metaKey || e.ctrlKey) && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        const newZoom = Math.round(Math.min(interfaceZoomRef.current + 0.05, 1.5) * 20) / 20;
        setInterfaceZoom(newZoom);
        toast(`Zoom ${Math.round(newZoom * 100)}%`, { id: "zoom", duration: 1500 });
        return;
      }

      // Cmd+- - Zoom out
      if ((e.metaKey || e.ctrlKey) && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        const newZoom = Math.round(Math.max(interfaceZoomRef.current - 0.05, 0.7) * 20) / 20;
        setInterfaceZoom(newZoom);
        toast(`Zoom ${Math.round(newZoom * 100)}%`, { id: "zoom", duration: 1500 });
        return;
      }

      // Cmd+0 - Reset zoom
      if ((e.metaKey || e.ctrlKey) && e.key === "0") {
        e.preventDefault();
        setInterfaceZoom(1.0);
        toast("Zoom 100%", { id: "zoom", duration: 1500 });
        return;
      }

      // Block other shortcuts when in settings view
      if (view === "settings") return;

      // Cmd+Shift+Enter - Toggle focus mode
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "Enter") {
        e.preventDefault();
        toggleFocusMode();
        return;
      }

      // Cmd+Shift+M - Toggle markdown source mode
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "m") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("toggle-source-mode"));
        return;
      }

      // Escape exits focus mode when not in editor
      if (e.key === "Escape" && focusMode && !isInEditor) {
        e.preventDefault();
        toggleFocusMode();
        return;
      }

      // Let dialogs handle their own keyboard events
      if (target.closest("[role='dialog'], [role='alertdialog']")) return;

      // Trap Tab/Shift+Tab
      if (e.key === "Tab") {
        e.preventDefault();
        return;
      }

      // Cmd+P - Open command palette
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key === "p") {
        e.preventDefault();
        setPaletteOpen(true);
        return;
      }

      // Cmd+Shift+P - Print
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("print-note"));
        return;
      }

      // Cmd+S - Save / Save As
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        const editor = editorRef.current;
        if (editor) {
          const manager = editor.storage.markdown?.manager;
          const markdown = manager
            ? manager.serialize(editor.getJSON())
            : editor.getText();
          saveRef.current(markdown);
        }
        return;
      }

      // Cmd+/ - Open keyboard shortcuts
      if ((e.metaKey || e.ctrlKey) && e.key === "/") {
        e.preventDefault();
        setShortcutsOpen(true);
        return;
      }

      // Cmd+O - Open file
      if ((e.metaKey || e.ctrlKey) && e.key === "o") {
        e.preventDefault();
        openFileDialog();
        return;
      }

      // Cmd+N - New file (Save As dialog)
      if ((e.metaKey || e.ctrlKey) && e.key === "n") {
        e.preventDefault();
        const saveDialog = async () => {
          try {
            const { save: saveFile } = await import("@tauri-apps/plugin-dialog");
            const path = await saveFile({
              filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
              defaultPath: "untitled.md",
            });
            if (path) {
              // Create empty file and load it
              await invoke("write_file", { path, contents: new TextEncoder().encode("# Untitled\n\n") });
              loadFile(path);
            }
          } catch (error) {
            console.error("Failed to create new file:", error);
            toast.error("Failed to create new file");
          }
        };
        saveDialog();
        return;
      }

      // Cmd+R - Reload file
      if ((e.metaKey || e.ctrlKey) && e.key === "r") {
        e.preventDefault();
        reload();
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    toggleSettings,
    toggleFocusMode,
    setInterfaceZoom,
    focusMode,
    view,
    openFileDialog,
    reload,
    loadFile,
  ]);

  const handleClosePalette = useCallback(() => {
    setPaletteOpen(false);
    editorRef.current?.commands.focus();
  }, []);

  const previewData: PreviewModeData | undefined = content !== null
    ? { content, title, filePath, modified, hasExternalChanges, reloadVersion, save, reload, autoSave: !!filePath }
    : undefined;

  return (
    <>
      <div className="h-full min-h-0 flex bg-bg text-text overflow-hidden">
        {view === "settings" ? (
          <SettingsPage onBack={closeSettings} />
        ) : (
          <Editor
            previewMode={previewData}
            onEditorReady={(editor) => {
              editorRef.current = editor;
            }}
            focusMode={focusMode}
          />
        )}
      </div>

      {/* Shared backdrop for command palette and AI modal */}
      {(paletteOpen || aiModalOpen) && (
        <div
          className="fixed inset-0 bg-text/50 backdrop-blur-sm z-40 animate-fade-in"
          onClick={() => {
            if (paletteOpen) handleClosePalette();
            if (aiModalOpen) setAiModalOpen(false);
          }}
        />
      )}

      <KeyboardShortcutsModal
        open={shortcutsOpen}
        onClose={() => setShortcutsOpen(false)}
      />

      <CommandPalette
        open={paletteOpen}
        onClose={handleClosePalette}
        onOpenSettings={toggleSettings}
        onOpenShortcuts={() => setShortcutsOpen(true)}
        onOpenAiModal={(provider) => {
          setAiProvider(provider);
          setAiModalOpen(true);
        }}
        focusMode={focusMode}
        onToggleFocusMode={toggleFocusMode}
        editorRef={editorRef}
        currentNote={previewData && previewData.content !== null ? { title: previewData.title, content: previewData.content } : null}
      />
      <AiEditModal
        open={aiModalOpen}
        provider={aiProvider}
        onBack={handleBackToPalette}
        onExecute={handleAiEdit}
        isExecuting={aiEditing}
      />

      {/* AI Editing Overlay */}
      {aiEditing && (
        <div className="fixed inset-0 bg-bg/50 backdrop-blur-sm z-50 flex items-center justify-center">
          <div className="flex items-center gap-2">
            <div className="text-sm font-medium text-text">
              {aiProvider === "codex"
                ? "Codex is editing..."
                : aiProvider === "opencode"
                  ? "OpenCode is editing..."
                  : aiProvider === "ollama"
                    ? "Ollama is editing..."
                    : "Claude is editing..."}
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function App() {
  // Cmd/Ctrl+W — close window
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "w") {
        e.preventDefault();
        getCurrentWindow().close().catch(console.error);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Add platform class for OS-specific styling
  useEffect(() => {
    const isMac = /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
    document.documentElement.classList.add(
      isMac ? "platform-mac" : "platform-other",
    );
  }, []);

  return (
    <ThemeProvider>
      <Toaster />
      <TooltipProvider>
        <AppContent />
      </TooltipProvider>
    </ThemeProvider>
  );
}

export default App;

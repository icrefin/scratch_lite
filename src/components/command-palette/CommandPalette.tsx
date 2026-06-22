import {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { useTheme } from "../../context/ThemeContext";
import * as aiService from "../../services/ai";
import { downloadPdf, downloadMarkdown } from "../../services/pdf";
import type { Editor } from "@tiptap/react";
import { CommandItem } from "../ui";
import { plainTextFromMarkdown } from "../../lib/plainText";
import {
  CopyIcon,
  DownloadIcon,
  SettingsIcon,
  SwatchIcon,
  ClaudeIcon,
  ZenIcon,
  MarkdownIcon,
  CodexIcon,
  OpenCodeIcon,
  OllamaIcon,
  KeyboardIcon,
} from "../icons";
import { mod, shift } from "../../lib/platform";
import type { AiProvider } from "../../services/ai";

interface Command {
  id: string;
  label: string;
  shortcut?: string;
  icon?: ReactNode;
  action: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onOpenSettings?: () => void;
  onOpenShortcuts?: () => void;
  onOpenAiModal?: (provider: AiProvider) => void;
  focusMode?: boolean;
  onToggleFocusMode?: () => void;
  editorRef?: React.RefObject<Editor | null>;
  currentNote?: { title: string; content: string } | null;
}

export function CommandPalette({
  open,
  onClose,
  onOpenSettings,
  onOpenShortcuts,
  onOpenAiModal,
  focusMode,
  onToggleFocusMode,
  editorRef,
  currentNote,
}: CommandPaletteProps) {
  const { setTheme } = useTheme();
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [availableAiProviders, setAvailableAiProviders] = useState<
    AiProvider[]
  >([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open || !currentNote) {
      setAvailableAiProviders([]);
      return;
    }

    let active = true;
    aiService
      .getAvailableAiProviders()
      .then((providers) => {
        if (active) {
          setAvailableAiProviders(providers);
        }
      })
      .catch((error) => {
        if (active) {
          console.error("Failed to discover AI providers:", error);
          setAvailableAiProviders([]);
        }
      });

    return () => {
      active = false;
    };
  }, [open, currentNote?.title]);

  const commands = useMemo<Command[]>(() => {
    const baseCommands: Command[] = [];

    // Add note-specific commands if a note is open
    if (currentNote) {
      const aiCommands: Command[] = onOpenAiModal
        ? availableAiProviders.map((provider) => {
            const action = () => {
              onOpenAiModal(provider);
              onClose();
            };

            if (provider === "codex") {
              return {
                id: "ai-edit-codex",
                label: "Edit with OpenAI Codex",
                icon: <CodexIcon className="w-4.5 h-4.5 fill-text-muted" />,
                action,
              };
            }

            if (provider === "opencode") {
              return {
                id: "ai-edit-opencode",
                label: "Edit with OpenCode",
                icon: (
                  <OpenCodeIcon className="w-4.5 h-4.5 fill-text-muted" />
                ),
                action,
              };
            }

            if (provider === "ollama") {
              return {
                id: "ai-edit-ollama",
                label: "Edit with Ollama",
                icon: <OllamaIcon className="w-4.5 h-4.5 fill-text-muted" />,
                action,
              };
            }

            return {
              id: "ai-edit-claude",
              label: "Edit with Claude Code",
              icon: <ClaudeIcon className="w-4.5 h-4.5 fill-text-muted" />,
              action,
            };
          })
        : [];

      baseCommands.push(
        ...aiCommands,
        {
          id: "copy-markdown",
          label: "Copy Markdown",
          icon: <CopyIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
          action: async () => {
            try {
              await invoke("copy_to_clipboard", { text: currentNote.content });
              toast.success("Copied as Markdown");
              onClose();
            } catch (error) {
              console.error("Failed to copy markdown:", error);
              toast.error("Failed to copy");
            }
          },
        },
        {
          id: "copy-plain",
          label: "Copy Plain Text",
          icon: <CopyIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
          action: async () => {
            try {
              const plainText = plainTextFromMarkdown(currentNote.content);
              await invoke("copy_to_clipboard", { text: plainText });
              toast.success("Copied as plain text");
              onClose();
            } catch (error) {
              console.error("Failed to copy plain text:", error);
              toast.error("Failed to copy");
            }
          },
        },
        {
          id: "copy-html",
          label: "Copy HTML",
          icon: <CopyIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
          action: async () => {
            try {
              if (!editorRef?.current) {
                toast.error("Editor not available");
                return;
              }
              const html = editorRef.current.getHTML();
              await invoke("copy_to_clipboard", { text: html });
              toast.success("Copied as HTML");
              onClose();
            } catch (error) {
              console.error("Failed to copy HTML:", error);
              toast.error("Failed to copy");
            }
          },
        },
        {
          id: "download-pdf",
          label: "Print as PDF",
          icon: <DownloadIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
          action: async () => {
            try {
              if (!editorRef?.current || !currentNote) {
                toast.error("Editor not available");
                return;
              }
              await downloadPdf(editorRef.current, currentNote.title);
              onClose();
            } catch (error) {
              console.error("Failed to open print dialog:", error);
              toast.error("Failed to open print dialog");
            }
          },
        },
        {
          id: "download-markdown",
          label: "Export Markdown",
          icon: <DownloadIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
          action: async () => {
            try {
              if (!currentNote) {
                toast.error("No note open");
                return;
              }
              let markdown = currentNote.content;
              const editorInstance = editorRef?.current;
              if (editorInstance) {
                const manager = editorInstance.storage.markdown?.manager;
                if (manager) {
                  markdown = manager.serialize(editorInstance.getJSON());
                  markdown = markdown.replace(/&nbsp;|&#160;/g, " ");
                } else {
                  markdown = editorInstance.getText();
                }
              }
              const saved = await downloadMarkdown(markdown, currentNote.title);
              if (saved) {
                toast.success("Markdown saved successfully");
                onClose();
              }
            } catch (error) {
              console.error("Failed to download markdown:", error);
              toast.error("Failed to save markdown");
            }
          },
        },
      );
    }

    // Focus mode and source toggle
    baseCommands.push(
      {
        id: "focus-mode",
        label: focusMode ? "Exit Focus Mode" : "Enter Focus Mode",
        shortcut: `${mod} ${shift} Enter`,
        icon: <ZenIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          onToggleFocusMode?.();
          onClose();
        },
      },
      {
        id: "toggle-source",
        label: "Toggle Markdown Source",
        shortcut: `${mod} ${shift} M`,
        icon: <MarkdownIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          window.dispatchEvent(new CustomEvent("toggle-source-mode"));
          onClose();
        },
      },
    );

    // Keyboard shortcuts, settings, and theme commands at the bottom
    baseCommands.push(
      {
        id: "keyboard-shortcuts",
        label: "Keyboard Shortcuts",
        shortcut: `${mod} /`,
        icon: <KeyboardIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          onOpenShortcuts?.();
          onClose();
        },
      },
      {
        id: "settings",
        label: "Settings",
        shortcut: `${mod} ,`,
        icon: <SettingsIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          onOpenSettings?.();
          onClose();
        },
      },
      {
        id: "theme-light",
        label: "Switch Theme to Light Mode",
        icon: <SwatchIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          setTheme("light");
          onClose();
        },
      },
      {
        id: "theme-dark",
        label: "Switch Theme to Dark Mode",
        icon: <SwatchIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          setTheme("dark");
          onClose();
        },
      },
      {
        id: "theme-system",
        label: "Switch Theme to System Mode",
        icon: <SwatchIcon className="w-4.5 h-4.5 stroke-[1.5]" />,
        action: () => {
          setTheme("system");
          onClose();
        },
      },
    );

    return baseCommands;
  }, [
    currentNote,
    onClose,
    onOpenSettings,
    onOpenAiModal,
    availableAiProviders,
    setTheme,
    focusMode,
    onToggleFocusMode,
    onOpenShortcuts,
    editorRef,
  ]);

  const filteredCommands = useMemo(() => {
    if (!query.trim()) return commands;
    const queryLower = query.toLowerCase();
    return commands.filter((cmd) =>
      cmd.label.toLowerCase().includes(queryLower),
    );
  }, [query, commands]);

  const allItems = useMemo(
    () =>
      filteredCommands.map((cmd) => ({
        type: "command" as const,
        id: cmd.id,
        label: cmd.label,
        shortcut: cmd.shortcut,
        icon: cmd.icon,
        action: cmd.action,
      })),
    [filteredCommands],
  );

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    if (listRef.current) {
      const selectedItem = listRef.current.querySelector(
        `[data-index="${selectedIndex}"]`,
      );
      selectedItem?.scrollIntoView({ block: "center", behavior: "smooth" });
    }
  }, [selectedIndex]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          e.stopPropagation();
          setSelectedIndex((i) => Math.min(i + 1, allItems.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          e.stopPropagation();
          setSelectedIndex((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
          e.preventDefault();
          e.stopPropagation();
          if (allItems[selectedIndex]) {
            allItems[selectedIndex].action();
          }
          break;
        case "Escape":
          e.preventDefault();
          e.stopPropagation();
          onClose();
          break;
      }
    },
    [allItems, selectedIndex, onClose],
  );

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center py-11 px-4 pointer-events-none">
      <div className="relative w-full h-full max-h-108 max-w-2xl bg-bg rounded-xl shadow-2xl overflow-hidden border border-border animate-slide-down flex flex-col pointer-events-auto">
        <div className="border-b border-border flex-none">
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a command..."
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            className="w-full px-4.5 py-3.5 text-[17px] bg-transparent outline-none text-text placeholder-text-muted/50"
          />
        </div>

        <div ref={listRef} className="overflow-y-auto h-full p-2.5 flex-1">
          {allItems.length === 0 ? (
            <div className="text-sm font-medium opacity-50 text-text-muted p-2">
              No results found
            </div>
          ) : (
            <div className="space-y-0.5">
              <div className="text-sm font-medium text-text-muted px-2.5 py-1.5">
                Commands
              </div>
              {filteredCommands.map((cmd, i) => (
                <div key={cmd.id} data-index={i}>
                  <CommandItem
                    label={cmd.label}
                    shortcut={cmd.shortcut}
                    icon={cmd.icon}
                    isSelected={selectedIndex === i}
                    onClick={cmd.action}
                  />
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

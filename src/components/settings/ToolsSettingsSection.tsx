import { useState, useEffect } from "react";
import { SpinnerIcon, CheckIcon, ClaudeIcon, CodexIcon, OpenCodeIcon, OllamaIcon } from "../icons";
import { AI_PROVIDER_ORDER, type AiProvider } from "../../services/ai";
import * as aiService from "../../services/ai";
import { mod } from "../../lib/platform";

const AI_PROVIDER_INFO: Record<
  AiProvider,
  {
    name: string;
    icon: React.ComponentType<{ className?: string }>;
    installUrl: string;
  }
> = {
  claude: {
    name: "Claude Code",
    icon: ClaudeIcon,
    installUrl: "https://code.claude.com/docs/en/quickstart",
  },
  codex: {
    name: "OpenAI Codex",
    icon: CodexIcon,
    installUrl: "https://github.com/openai/codex",
  },
  opencode: {
    name: "OpenCode",
    icon: OpenCodeIcon,
    installUrl: "https://opencode.ai",
  },
  ollama: {
    name: "Ollama",
    icon: OllamaIcon,
    installUrl: "https://ollama.com",
  },
};

export function ToolsSettingsSection() {
  const [aiProviders, setAiProviders] = useState<AiProvider[]>([]);
  const [aiProvidersLoading, setAiProvidersLoading] = useState(true);

  useEffect(() => {
    aiService
      .getAvailableAiProviders()
      .then(setAiProviders)
      .catch(() => setAiProviders([]))
      .finally(() => setAiProvidersLoading(false));
  }, []);

  return (
    <div className="space-y-8 py-8">
      {/* AI Providers */}
      <section className="pb-2">
        <h2 className="text-xl font-medium mb-0.5">AI Providers</h2>
        <p className="text-sm text-text-muted mb-4">
          Edit notes with AI from the command palette ({mod}P while editing a
          note)
        </p>

        {aiProvidersLoading ? (
          <div className="flex items-center gap-2 p-3">
            <SpinnerIcon className="w-4 h-4 animate-spin text-text-muted" />
            <span className="text-sm text-text-muted">
              Detecting installed providers...
            </span>
          </div>
        ) : (
          <div className="space-y-2">
            {AI_PROVIDER_ORDER.map((provider) => {
              const installed = aiProviders.includes(provider);
              const info = AI_PROVIDER_INFO[provider];
              return (
                <div
                  key={provider}
                  className="flex items-center justify-between p-3 rounded-[10px] border border-border"
                >
                  <div className="flex items-center gap-2.5">
                    <info.icon className="w-4.5 h-4.5 text-text-muted" />
                    <span className="text-sm font-medium">{info.name}</span>
                  </div>
                  {installed ? (
                    <span className="flex items-center gap-1.25 text-sm text-text-muted">
                      Installed
                      <span className="h-4.5 w-4.5 bg-bg-emphasis rounded-full flex items-center justify-center">
                        <CheckIcon className="w-3 h-3 stroke-[2.2]" />
                      </span>
                    </span>
                  ) : (
                    <a
                      href={info.installUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm text-text font-medium hover:text-text-muted transition-colors cursor-pointer"
                    >
                      Install
                    </a>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

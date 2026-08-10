"use client";

/**
 * AiSettingsDialog — provider/model/URL/key configuration (Phase 0.2 / G.0.2).
 *
 * Lets the user pick an AI provider (Ollama / OpenAI-compat / Z.ai), enter
 * the endpoint URL + API key + model name, and test the connection.
 *
 * The chosen config is persisted in the Zustand store (and thereby in the
 * Tauri Store plugin — see `tauri-store-adapter.ts`), so all AI dialogs
 * (AIDialog, AssistantDialog, future Layer G LLM Hypotheses tab) can read
 * it via `useLitStore.getState().aiProviderConfig` and forward it to the
 * Tauri invoke() call.
 *
 * Why this is critical: before this dialog existed, AI commands always
 * failed at runtime with "missing field `provider`" because the dialogs
 * bypassed the typed wrappers in `tauri-commands.ts` and called `callApi()`
 * with a hand-crafted payload that omitted `provider` (see subagent
 * reports 05, 08, 11, 17 in docs/layer-g-planning/subagent-reports/).
 */

import { useState, useEffect } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Loader2, Sparkles, CheckCircle2, AlertTriangle, Cpu } from "lucide-react";
import { useLitStore, type AiProviderConfig, defaultOllamaConfig } from "@/lib/litgraph/store";
import { aiTestConnection, aiListOllamaModels } from "@/lib/tauri-commands";
import { callApi } from "@/lib/litgraph/api";

type ProviderType = "ollama" | "openaicompat" | "zai";

const PROVIDER_LABELS: Record<ProviderType, string> = {
  ollama: "Ollama (локально)",
  openaicompat: "OpenAI-compatible API",
  zai: "Z.ai API",
};

const PROVIDER_DESCRIPTIONS: Record<ProviderType, string> = {
  ollama:
    "Запуск локальної моделі через Ollama (https://ollama.com). Безкоштовно, приватно, працює офлайн.",
  openaicompat:
    "Сумісний з OpenAI API endpoint: OpenAI, Groq, Together AI, OpenRouter, локальний vLLM тощо.",
  zai:
    "Хмарний API від Z.ai (glm-4.6, glm-4.5). Потрібен API-ключ із https://z.ai.",
};

export function AiSettingsDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const aiProviderConfig = useLitStore((s) => s.aiProviderConfig);
  const setAiProviderConfig = useLitStore((s) => s.setAiProviderConfig);

  // Local form state — initialised from store, only committed on save.
  const [providerType, setProviderType] = useState<ProviderType>("ollama");
  const [url, setUrl] = useState("http://localhost:11434");
  const [endpoint, setEndpoint] = useState("https://api.openai.com/v1");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("llama3.1");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<
    { ok: true; message: string } | { ok: false; message: string } | null
  >(null);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);

  // Sync local form state from store when dialog opens
  useEffect(() => {
    if (!open) return;
    setTestResult(null);
    setOllamaModels([]);
    if (aiProviderConfig) {
      const cfg = aiProviderConfig;
      setProviderType(cfg.type);
      if (cfg.type === "ollama") {
        setUrl(cfg.url);
        setModel(cfg.model);
      } else if (cfg.type === "openaicompat") {
        setEndpoint(cfg.endpoint);
        setApiKey(cfg.apiKey);
        setModel(cfg.model);
      } else if (cfg.type === "zai") {
        setApiKey(cfg.apiKey);
        setModel(cfg.model);
      }
    } else {
      // First run — load defaults
      const def = defaultOllamaConfig();
      setProviderType("ollama");
      setUrl(def.url);
      setModel(def.model);
    }
  }, [open, aiProviderConfig]);

  // When provider type changes, pick sensible defaults
  useEffect(() => {
    if (!open) return;
    if (providerType === "ollama" && !model) setModel("llama3.1");
    if (providerType === "openaicompat" && !model) setModel("gpt-4o-mini");
    if (providerType === "zai" && !model) setModel("glm-4.6");
  }, [providerType, open, model]);

  function buildConfig(): AiProviderConfig {
    switch (providerType) {
      case "ollama":
        return { type: "ollama", url: url.trim() || "http://localhost:11434", model: model.trim() || "llama3.1" };
      case "openaicompat":
        return {
          type: "openaicompat",
          endpoint: endpoint.trim() || "https://api.openai.com/v1",
          apiKey: apiKey.trim(),
          model: model.trim() || "gpt-4o-mini",
        };
      case "zai":
        return {
          type: "zai",
          apiKey: apiKey.trim(),
          model: model.trim() || "glm-4.6",
        };
    }
  }

  async function handleTestConnection() {
    setTesting(true);
    setTestResult(null);
    try {
      const cfg = buildConfig();
      // Use the typed wrapper from tauri-commands.ts — this ensures we pass
      // the provider in the exact shape Rust expects (no "missing field
      // `provider`" runtime error).
      const ok = await aiTestConnection(cfg);
      if (ok) {
        setTestResult({
          ok: true,
          message: `З'єднання успішне — модель '${cfg.type === "ollama" ? cfg.url : cfg.model}' відповідає.`,
        });
      } else {
        setTestResult({ ok: false, message: "Тест не пройшов — модель не відповідає." });
      }
    } catch (err) {
      setTestResult({
        ok: false,
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTesting(false);
    }
  }

  async function handleFetchOllamaModels() {
    if (providerType !== "ollama") return;
    setTesting(true);
    try {
      // Try via the typed wrapper (Tauri env)
      const models = await aiListOllamaModels(url.trim() || "http://localhost:11434");
      setOllamaModels(models);
      if (models.length === 0) {
        setTestResult({
          ok: false,
          message: "Ollama запущено, але моделі не встановлені. Запустіть `ollama pull llama3.1`.",
        });
      }
    } catch (err) {
      // Web-preview fallback — callApi goes through fetch() to /api/ai/ollama-models
      try {
        const models = await callApi<string[]>("ai_list_ollama_models", "/api/ai/ollama-models", {
          url: url.trim() || "http://localhost:11434",
        });
        setOllamaModels(models);
      } catch (err2) {
        setTestResult({
          ok: false,
          message: `Не вдалося отримати список моделей: ${err2 instanceof Error ? err2.message : String(err2)}`,
        });
      }
    } finally {
      setTesting(false);
    }
  }

  function handleSave() {
    const cfg = buildConfig();
    setAiProviderConfig(cfg);
    onClose();
  }

  function handleClear() {
    setAiProviderConfig(null);
    onClose();
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Cpu className="w-5 h-5 text-violet-600" />
            Налаштування AI
          </DialogTitle>
          <DialogDescription>
            Оберіть провайдера та модель. Конфігурація зберігається в Tauri Store і
            використовується всіма AI-діалогами (помічник, дописування глави, Layer G).
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* Provider picker */}
          <div className="space-y-1.5">
            <Label className="text-xs text-stone-500">Провайдер</Label>
            <Select value={providerType} onValueChange={(v) => setProviderType(v as ProviderType)}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(PROVIDER_LABELS) as ProviderType[]).map((p) => (
                  <SelectItem key={p} value={p}>
                    {PROVIDER_LABELS[p]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-[11px] text-stone-500 leading-relaxed">
              {PROVIDER_DESCRIPTIONS[providerType]}
            </p>
          </div>

          {/* Ollama-specific fields */}
          {providerType === "ollama" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Ollama URL</Label>
                <Input
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="http://localhost:11434"
                  className="text-sm font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label className="text-xs text-stone-500">Модель</Label>
                  <button
                    type="button"
                    onClick={handleFetchOllamaModels}
                    disabled={testing}
                    className="text-[11px] text-violet-600 hover:text-violet-800 disabled:opacity-50"
                  >
                    {testing ? "Завантаження…" : "Оновити список моделей"}
                  </button>
                </div>
                {ollamaModels.length > 0 ? (
                  <Select value={model} onValueChange={setModel}>
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder="Оберіть модель" />
                    </SelectTrigger>
                    <SelectContent>
                      {ollamaModels.map((m) => (
                        <SelectItem key={m} value={m}>
                          {m}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : (
                  <Input
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="llama3.1"
                    className="text-sm font-mono"
                  />
                )}
              </div>
            </>
          )}

          {/* OpenAI-compat fields */}
          {providerType === "openaicompat" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">API Endpoint</Label>
                <Input
                  value={endpoint}
                  onChange={(e) => setEndpoint(e.target.value)}
                  placeholder="https://api.openai.com/v1"
                  className="text-sm font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">API Key</Label>
                <Input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-..."
                  className="text-sm font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Модель</Label>
                <Input
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder="gpt-4o-mini"
                  className="text-sm font-mono"
                />
              </div>
            </>
          )}

          {/* Z.ai fields */}
          {providerType === "zai" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Z.ai API Key</Label>
                <Input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Отримайте на https://z.ai"
                  className="text-sm font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Модель</Label>
                <Select value={model} onValueChange={setModel}>
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="glm-4.6">glm-4.6 (найновіша)</SelectItem>
                    <SelectItem value="glm-4.5">glm-4.5</SelectItem>
                    <SelectItem value="glm-4.5-air">glm-4.5-air (швидка)</SelectItem>
                    <SelectItem value="glm-4.5-flash">glm-4.5-flash (безкоштовна)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </>
          )}

          {/* Test connection result */}
          {testResult && (
            <div
              className={`rounded-md border p-3 text-sm flex items-start gap-2 ${
                testResult.ok
                  ? "bg-emerald-50 border-emerald-200 text-emerald-700"
                  : "bg-red-50 border-red-200 text-red-700"
              }`}
            >
              {testResult.ok ? (
                <CheckCircle2 className="w-4 h-4 shrink-0 mt-0.5" />
              ) : (
                <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
              )}
              <div className="flex-1 break-words">{testResult.message}</div>
            </div>
          )}

          {/* Privacy notice */}
          <div className="rounded-md bg-amber-50 border border-amber-200 p-3 text-[11px] text-amber-800 leading-relaxed">
            🔒 API-ключі зберігаються локально у Tauri Store (
            <code className="font-mono">~/.local/share/litgraph/store.bin</code>) і ніколи не
            передаються стороннім серверам, окрім самого обраного провайдера.
          </div>
        </div>

        <DialogFooter className="border-t pt-4 flex-wrap gap-2">
          <Button variant="outline" onClick={handleTestConnection} disabled={testing}>
            {testing ? (
              <>
                <Loader2 className="w-4 h-4 mr-1.5 animate-spin" />
                Тестуємо…
              </>
            ) : (
              <>
                <Sparkles className="w-4 h-4 mr-1.5" />
                Перевірити з'єднання
              </>
            )}
          </Button>
          <div className="flex-1" />
          {aiProviderConfig && (
            <Button variant="ghost" onClick={handleClear} className="text-red-600 hover:text-red-700">
              Очистити конфіг
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            Скасувати
          </Button>
          <Button onClick={handleSave}>Зберегти</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

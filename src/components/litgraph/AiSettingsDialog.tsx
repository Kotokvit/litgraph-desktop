"use client";

import { useState, useEffect } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Loader2, CheckCircle2, XCircle } from "lucide-react";

interface AiProviderConfig {
  type: "ollama" | "openaicompat" | "zai";
  // Ollama
  url?: string;
  model?: string;
  // OpenAI-compat
  endpoint?: string;
  apiKey?: string;
}

export function AiSettingsDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [config, setConfig] = useState<AiProviderConfig>({
    type: "ollama",
    url: "http://localhost:11434",
    model: "llama3.1",
  });
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<"ok" | "fail" | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Загружаем текущие настройки при открытии
  useEffect(() => {
    if (!open) return;
    (async () => {
      try {
        const { Store } = await import("@tauri-apps/plugin-store");
        const store = await Store.load("config.json");
        const saved = await store.get<AiProviderConfig>("aiProvider");
        if (saved) {
          setConfig(saved);
          if (saved.type === "ollama" && saved.url) {
            loadOllamaModels(saved.url);
          }
        }
      } catch (err) {
        console.error("Failed to load AI config:", err);
      }
    })();
  }, [open]);

  async function loadOllamaModels(url: string) {
    setLoadingModels(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const models = await invoke<string[]>("ai_list_ollama_models", { url });
      setOllamaModels(models);
    } catch (err) {
      console.error("Failed to load Ollama models:", err);
      setOllamaModels([]);
    } finally {
      setLoadingModels(false);
    }
  }

  async function handleTest() {
    setTesting(true);
    setTestResult(null);
    setTestError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("ai_test_connection", { provider: config });
      setTestResult("ok");
    } catch (err) {
      setTestResult("fail");
      setTestError(String(err));
    } finally {
      setTesting(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      const { Store } = await import("@tauri-apps/plugin-store");
      const store = await Store.load("config.json");
      await store.set("aiProvider", config);
      await store.save();
      onClose();
    } catch (err) {
      console.error("Failed to save AI config:", err);
      alert("Не удалось сохранить: " + String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Настройки AI</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {/* Тип провайдера */}
          <div className="space-y-1.5">
            <Label className="text-xs text-stone-500">Провайдер</Label>
            <div className="grid grid-cols-3 gap-2">
              <button
                onClick={() => setConfig({ ...config, type: "ollama" })}
                className={`p-2 rounded-md border text-xs font-medium transition-colors ${
                  config.type === "ollama"
                    ? "bg-stone-800 text-white border-stone-800"
                    : "bg-white text-stone-700 border-stone-200 hover:bg-stone-50"
                }`}
              >
                Ollama
                <div className="text-[10px] font-normal opacity-70">локально</div>
              </button>
              <button
                onClick={() => setConfig({ ...config, type: "openaicompat" })}
                className={`p-2 rounded-md border text-xs font-medium transition-colors ${
                  config.type === "openaicompat"
                    ? "bg-stone-800 text-white border-stone-800"
                    : "bg-white text-stone-700 border-stone-200 hover:bg-stone-50"
                }`}
              >
                OpenAI-compat
                <div className="text-[10px] font-normal opacity-70">свой ключ</div>
              </button>
              <button
                onClick={() => setConfig({ ...config, type: "zai" })}
                className={`p-2 rounded-md border text-xs font-medium transition-colors ${
                  config.type === "zai"
                    ? "bg-stone-800 text-white border-stone-800"
                    : "bg-white text-stone-700 border-stone-200 hover:bg-stone-50"
                }`}
              >
                Z.ai
                <div className="text-[10px] font-normal opacity-70">через SDK</div>
              </button>
            </div>
          </div>

          {/* Ollama */}
          {config.type === "ollama" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">URL сервера Ollama</Label>
                <Input
                  value={config.url || ""}
                  onChange={(e) => setConfig({ ...config, url: e.target.value })}
                  placeholder="http://localhost:11434"
                  className="text-sm"
                />
              </div>
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <Label className="text-xs text-stone-500">Модель</Label>
                  <button
                    onClick={() => config.url && loadOllamaModels(config.url)}
                    className="text-[10px] text-amber-700 hover:underline"
                    disabled={loadingModels}
                  >
                    {loadingModels ? "Загрузка…" : "Обновить список"}
                  </button>
                </div>
                {ollamaModels.length > 0 ? (
                  <select
                    value={config.model || ""}
                    onChange={(e) => setConfig({ ...config, model: e.target.value })}
                    className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm"
                  >
                    {ollamaModels.map((m) => (
                      <option key={m} value={m}>{m}</option>
                    ))}
                  </select>
                ) : (
                  <Input
                    value={config.model || ""}
                    onChange={(e) => setConfig({ ...config, model: e.target.value })}
                    placeholder="llama3.1, qwen2.5, mistral, ..."
                    className="text-sm"
                  />
                )}
              </div>
              <p className="text-[10px] text-stone-400 leading-relaxed">
                Установите Ollama: <code className="bg-stone-100 px-1 rounded">curl -fsSL https://ollama.com/install.sh | sh</code>
                <br />
                Скачайте модель: <code className="bg-stone-100 px-1 rounded">ollama pull llama3.1</code>
                <br />
                Запустите сервер: <code className="bg-stone-100 px-1 rounded">ollama serve</code>
              </p>
            </>
          )}

          {/* OpenAI-compat */}
          {config.type === "openaicompat" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Endpoint</Label>
                <Input
                  value={config.endpoint || ""}
                  onChange={(e) => setConfig({ ...config, endpoint: e.target.value })}
                  placeholder="https://api.openai.com/v1"
                  className="text-sm"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">API Key</Label>
                <Input
                  type="password"
                  value={config.apiKey || ""}
                  onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
                  placeholder="sk-..."
                  className="text-sm"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Модель</Label>
                <Input
                  value={config.model || ""}
                  onChange={(e) => setConfig({ ...config, model: e.target.value })}
                  placeholder="gpt-4o-mini, llama-3.1-70b, ..."
                  className="text-sm"
                />
              </div>
              <p className="text-[10px] text-stone-400 leading-relaxed">
                Работает с OpenAI, Groq, OpenRouter, Together AI, LiteLLM, vLLM,
                и любым другим OpenAI-совместимым сервером.
              </p>
            </>
          )}

          {/* Z.ai */}
          {config.type === "zai" && (
            <>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Z.ai API Key</Label>
                <Input
                  type="password"
                  value={config.apiKey || ""}
                  onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
                  placeholder="zai-..."
                  className="text-sm"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Модель</Label>
                <Input
                  value={config.model || ""}
                  onChange={(e) => setConfig({ ...config, model: e.target.value })}
                  placeholder="glm-4.6"
                  className="text-sm"
                />
              </div>
              <p className="text-[10px] text-stone-400 leading-relaxed">
                Получить ключ: https://z.ai
              </p>
            </>
          )}

          {/* Тест соединения */}
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleTest}
              disabled={testing}
            >
              {testing ? (
                <>
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  Проверка…
                </>
              ) : (
                "Проверить соединение"
              )}
            </Button>
            {testResult === "ok" && (
              <span className="text-xs text-emerald-600 flex items-center gap-1">
                <CheckCircle2 className="w-3.5 h-3.5" />
                Соединение работает
              </span>
            )}
            {testResult === "fail" && (
              <span className="text-xs text-red-600 flex items-center gap-1">
                <XCircle className="w-3.5 h-3.5" />
                Ошибка
              </span>
            )}
          </div>
          {testError && (
            <p className="text-[10px] text-red-600 break-all">{testError}</p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Отмена
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "Сохранение…" : "Сохранить"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

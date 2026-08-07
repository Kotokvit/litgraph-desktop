"use client";

import { useState } from "react";
import { callApi } from "@/lib/litgraph/api";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Loader2, Sparkles, AlertTriangle, Save, Copy, Download } from "lucide-react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES } from "@/lib/litgraph/types";
import { downloadFile, slugify } from "@/lib/litgraph/export";

type AIMode = "continue-chapter" | "analyze-plot";

interface AIResultState {
  loading: boolean;
  mode: AIMode | null;
  result: string | null;
  error: string | null;
  meta?: Record<string, unknown>;
}

export function AIDialog({
  open,
  mode,
  onClose,
}: {
  open: boolean;
  mode: AIMode | null;
  onClose: () => void;
}) {
  const exportProject = useLitStore((s) => s.exportProject);
  const title = useLitStore((s) => s.title);
  const nodes = useLitStore((s) => s.nodes);
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const addNode = useLitStore((s) => s.addNode);
  const updateNodeData = useLitStore((s) => s.updateNodeData);

  const [customPrompt, setCustomPrompt] = useState("");
  const [focus, setFocus] = useState<string>("all");
  const [state, setState] = useState<AIResultState>({
    loading: false, mode: null, result: null, error: null,
  });

  // Сбрасываем состояние при смене режима
  const modeKey = mode ?? "none";
  const [lastModeKey, setLastModeKey] = useState<string>(modeKey);
  if (modeKey !== lastModeKey) {
    setLastModeKey(modeKey);
    setState({ loading: false, mode: null, result: null, error: null });
    setCustomPrompt("");
  }

  async function runAI() {
    if (!mode) return;
    setState({ loading: true, mode, result: null, error: null });
    try {
      const project = exportProject();
      const cmdName = mode === "continue-chapter" ? "ai_continue_chapter" : "ai_analyze_plot";
      const endpoint = mode === "continue-chapter" ? "/api/ai/continue-chapter" : "/api/ai/analyze-plot";

      const payload: Record<string, unknown> = { project };
      if (mode === "continue-chapter") {
        if (selectedNodeId) payload.fromChapterId = selectedNodeId;
        if (customPrompt.trim()) payload.customPrompt = customPrompt.trim();
      } else {
        payload.focus = focus;
      }

      const text = await callApi<string>(cmdName, endpoint, payload);
      setState({
        loading: false,
        mode,
        result: text,
        error: null,
      });
    } catch (err) {
      setState({
        loading: false,
        mode,
        result: null,
        error: (err as Error).message,
      });
    }
  }

  function saveAsChapter() {
    if (!state.result || mode !== "continue-chapter") return;
    // Найдём номер последней главы
    const chapterNodes = nodes.filter((n) => n.type === "chapter");
    const nums = chapterNodes
      .map((n) => {
        const m = n.data.title.match(/Глава\s+(\d+)/i);
        return m ? parseInt(m[1], 10) : 0;
      })
      .sort((a, b) => a - b);
    const nextNum = (nums[nums.length - 1] || 0) + 1;
    const cfg = NODE_TYPES.chapter;
    const pos = {
      x: 600 + (Math.random() - 0.5) * 100,
      y: 60 + chapterNodes.length * 130,
    };
    const id = addNode("chapter", pos);
    updateNodeData(id, {
      title: `Глава ${nextNum}: (AI-черновик)`,
      body: state.result.slice(0, 300) + "…",
      fullText: state.result,
      tags: ["AI", "черновик"],
      meta: { wordCount: state.result.split(/\s+/).length },
    });
    void cfg; // заглушка
    onClose();
  }

  function copyToClipboard() {
    if (state.result) {
      navigator.clipboard.writeText(state.result);
    }
  }

  function downloadTxt() {
    if (!state.result) return;
    const fname =
      mode === "continue-chapter"
        ? `${slugify(title)}-ai-chapter.txt`
        : `${slugify(title)}-ai-analysis.txt`;
    downloadFile(state.result, fname);
  }

  const isContinue = mode === "continue-chapter";
  const isAnalyze = mode === "analyze-plot";

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl max-h-[90vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {isContinue ? (
              <>
                <Sparkles className="w-5 h-5 text-amber-500" />
                Дописать следующую главу
              </>
            ) : isAnalyze ? (
              <>
                <AlertTriangle className="w-5 h-5 text-rose-500" />
                Анализ слабых мест сюжета
              </>
            ) : null}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll pr-1 space-y-4">
          {!state.result && !state.loading && (
            <>
              {isContinue && (
                <div className="space-y-3">
                  <p className="text-sm text-stone-600 leading-relaxed">
                    AI возьмёт последние 2-3 главы как контекст, посмотрит на
                    персонажей, локации и активные сюжетные точки — и допишет
                    следующую главу в стиле автора.
                  </p>
                  {selectedNodeId && (
                    <div className="rounded-md bg-amber-50 border border-amber-200 p-2.5 text-xs text-amber-800">
                      💡 Сейчас выбрана нода — AI будет считать её последней и
                      продолжит с неё.
                    </div>
                  )}
                  <div className="space-y-1.5">
                    <Label className="text-xs text-stone-500">
                      Дополнительные указания (необязательно)
                    </Label>
                    <Textarea
                      value={customPrompt}
                      onChange={(e) => setCustomPrompt(e.target.value)}
                      placeholder="Например: «введи нового персонажа», «сделай кульминацию», «больше диалогов»…"
                      className="min-h-[80px] text-sm"
                    />
                  </div>
                </div>
              )}
              {isAnalyze && (
                <div className="space-y-3">
                  <p className="text-sm text-stone-600 leading-relaxed">
                    AI проанализирует структуру графа: персонажей, сюжетные точки,
                    конфликты, темп по главам — и найдёт слабые места.
                  </p>
                  <div className="space-y-1.5">
                    <Label className="text-xs text-stone-500">Фокус анализа</Label>
                    <select
                      value={focus}
                      onChange={(e) => setFocus(e.target.value)}
                      className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm"
                    >
                      <option value="all">Полный анализ</option>
                      <option value="plot">Только сюжет</option>
                      <option value="characters">Только персонажи</option>
                      <option value="pacing">Только темп и ритм</option>
                    </select>
                  </div>
                </div>
              )}
            </>
          )}

          {state.loading && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <Loader2 className="w-8 h-8 text-amber-500 animate-spin" />
              <p className="text-sm text-stone-500">
                {isContinue
                  ? "AI дописывает главу… (обычно 30-60 секунд)"
                  : "AI анализирует сюжет… (обычно 20-40 секунд)"}
              </p>
            </div>
          )}

          {state.error && (
            <div className="rounded-md bg-red-50 border border-red-200 p-3 text-sm text-red-700">
              ❌ {state.error}
            </div>
          )}

          {state.result && (
            <div className="space-y-3">
              {state.meta && (
                <div className="flex flex-wrap gap-2 text-[10px] text-stone-500">
                  {Object.entries(state.meta).map(([k, v]) => (
                    <span key={k} className="bg-stone-100 px-2 py-0.5 rounded-full">
                      {k}: {String(v)}
                    </span>
                  ))}
                </div>
              )}
              <Textarea
                value={state.result}
                readOnly
                className="min-h-[400px] font-serif text-sm leading-relaxed"
                style={{ fontFamily: "Georgia, 'Times New Roman', serif" }}
              />
            </div>
          )}
        </div>

        <DialogFooter className="border-t pt-3 gap-2 flex-wrap">
          {!state.result && !state.loading && (
            <Button onClick={runAI} className="mr-auto">
              {isContinue ? (
                <>
                  <Sparkles className="w-4 h-4 mr-1.5" />
                  Сгенерировать
                </>
              ) : (
                <>
                  <AlertTriangle className="w-4 h-4 mr-1.5" />
                  Запустить анализ
                </>
              )}
            </Button>
          )}
          {state.result && (
            <>
              {isContinue && (
                <Button onClick={saveAsChapter} className="mr-auto">
                  <Save className="w-4 h-4 mr-1.5" />
                  Сохранить как новую главу
                </Button>
              )}
              <Button variant="outline" onClick={copyToClipboard}>
                <Copy className="w-4 h-4 mr-1.5" />
                Копировать
              </Button>
              <Button variant="outline" onClick={downloadTxt}>
                <Download className="w-4 h-4 mr-1.5" />
                Скачать
              </Button>
            </>
          )}
          <Button variant="ghost" onClick={onClose}>
            Закрыть
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

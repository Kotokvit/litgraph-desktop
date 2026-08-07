"use client";

import { useState, useRef, useEffect } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Loader2, Sparkles, Send, User, Bot, Trash2 } from "lucide-react";
import { useLitStore } from "@/lib/litgraph/store";

interface Message {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

const PRESETS = [
  { label: "Анализ персонажа", text: "Проанализируй дугу главного героя: где она начинается, какие поворотные точки, чем заканчивается. Что можно усилить?" },
  { label: "Найди дыры в сюжете", text: "Найди логические нестыковки и нераскрытые линии в сюжете. Конкретно — с номерами глав." },
  { label: "Идея для следующей главы", text: "Предложи 3 разных варианта развития сюжета для следующей главы. С коротким описанием каждого." },
  { label: "Темы и мотивы", text: "Какие сквозные темы и мотивы прослеживаются в произведении? Как они развиваются от главы к главе?" },
  { label: "Стиль и атмосфера", text: "Проанализируй стиль и атмосферу текста. Какие приёмы использует автор? Что работает особенно хорошо?" },
  { label: "Хронометраж", text: "Проанализируй темп и ритм: какие главы провисают, где слишком быстро/медленно. Дай рекомендации." },
];

export function AssistantDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const exportProject = useLitStore((s) => s.exportProject);
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const nodes = useLitStore((s) => s.nodes);

  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Прокрутка вниз при новых сообщениях
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, loading]);

  async function send(text?: string) {
    const content = (text ?? input).trim();
    if (!content || loading) return;

    const userMsg: Message = {
      role: "user",
      content,
      timestamp: Date.now(),
    };
    const newMessages = [...messages, userMsg];
    setMessages(newMessages);
    setInput("");
    setLoading(true);
    setError(null);

    try {
      const project = exportProject();
      const res = await fetch("/api/ai/assistant", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          project,
          message: content,
          history: messages.map((m) => ({ role: m.role, content: m.content })),
          selectedNodeId,
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Неизвестная ошибка");

      const aiMsg: Message = {
        role: "assistant",
        content: data.text,
        timestamp: Date.now(),
      };
      setMessages([...newMessages, aiMsg]);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }

  function clearChat() {
    if (confirm("Очистить всю историю чата?")) {
      setMessages([]);
      setError(null);
    }
  }

  // Выбранная нода для контекста
  const selectedNode = selectedNodeId
    ? nodes.find((n) => n.id === selectedNodeId)
    : null;

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl max-h-[92vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-amber-500" />
            AI-помощник
            <span className="text-xs text-stone-400 font-normal ml-2">
              знает структуру всего графа
            </span>
          </DialogTitle>
        </DialogHeader>

        {/* Контекст выбранной ноды */}
        {selectedNode && (
          <div className="rounded-md bg-amber-50 border border-amber-200 p-2 text-xs text-amber-800 flex items-center gap-2">
            <Sparkles className="w-3 h-3 shrink-0" />
            <span>
              В контексте: <strong>{selectedNode.data.title}</strong> ({selectedNode.type}).
              AI будет учитывать содержимое этой ноды.
            </span>
          </div>
        )}

        {/* Пресеты (когда нет сообщений) */}
        {messages.length === 0 && (
          <div className="space-y-2">
            <div className="text-xs text-stone-500 uppercase tracking-wider">
              Быстрые вопросы
            </div>
            <div className="flex flex-wrap gap-1.5">
              {PRESETS.map((p) => (
                <button
                  key={p.label}
                  onClick={() => send(p.text)}
                  disabled={loading}
                  className="text-xs px-2.5 py-1.5 rounded-md bg-stone-100 hover:bg-stone-200 text-stone-700 transition-colors disabled:opacity-50"
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* История чата */}
        <div
          ref={scrollRef}
          className="flex-1 overflow-y-auto lit-scroll space-y-3 min-h-[300px] max-h-[500px]"
        >
          {messages.length === 0 && !loading && (
            <div className="text-center py-12 text-stone-400 text-sm">
              <Bot className="w-10 h-10 mx-auto mb-3 opacity-40" />
              Задайте любой вопрос о произведении — анализ, идея, проверка логики, генерация фрагмента…
              <br />
              AI видит все {nodes.length} нод графа и связи между ними.
            </div>
          )}

          {messages.map((m, i) => (
            <div
              key={i}
              className={`flex gap-2.5 ${m.role === "user" ? "flex-row-reverse" : ""}`}
            >
              <div
                className={`w-7 h-7 rounded-full flex items-center justify-center shrink-0 ${
                  m.role === "user"
                    ? "bg-stone-700 text-white"
                    : "bg-gradient-to-br from-amber-500 to-stone-700 text-white"
                }`}
              >
                {m.role === "user" ? <User className="w-3.5 h-3.5" /> : <Bot className="w-3.5 h-3.5" />}
              </div>
              <div
                className={`flex-1 max-w-[85%] rounded-lg p-3 text-sm leading-relaxed whitespace-pre-wrap ${
                  m.role === "user"
                    ? "bg-stone-100 text-stone-800"
                    : "bg-white border border-stone-200 text-stone-700"
                }`}
                style={m.role === "assistant" ? { fontFamily: "Georgia, 'Times New Roman', serif" } : undefined}
              >
                {m.content}
              </div>
            </div>
          ))}

          {loading && (
            <div className="flex gap-2.5">
              <div className="w-7 h-7 rounded-full bg-gradient-to-br from-amber-500 to-stone-700 text-white flex items-center justify-center shrink-0">
                <Bot className="w-3.5 h-3.5" />
              </div>
              <div className="bg-white border border-stone-200 rounded-lg p-3 flex items-center gap-2 text-sm text-stone-500">
                <Loader2 className="w-4 h-4 animate-spin" />
                Думаю…
              </div>
            </div>
          )}

          {error && (
            <div className="rounded-md bg-red-50 border border-red-200 p-2.5 text-sm text-red-700">
              ❌ {error}
            </div>
          )}
        </div>

        {/* Поле ввода */}
        <div className="border-t pt-3 space-y-2">
          <Textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                send();
              }
            }}
            placeholder="Спросите что угодно о произведении… (Ctrl+Enter — отправить)"
            className="min-h-[60px] max-h-[150px] resize-y text-sm"
            disabled={loading}
          />
          <div className="flex items-center gap-2">
            <Button
              onClick={() => send()}
              disabled={loading || !input.trim()}
              className="ml-auto"
              size="sm"
            >
              <Send className="w-3.5 h-3.5 mr-1.5" />
              Отправить
            </Button>
            {messages.length > 0 && (
              <Button
                onClick={clearChat}
                disabled={loading}
                variant="ghost"
                size="sm"
                className="text-red-500 hover:text-red-700 hover:bg-red-50"
              >
                <Trash2 className="w-3.5 h-3.5 mr-1" />
                Очистить
              </Button>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

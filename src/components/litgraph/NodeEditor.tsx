"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  X, Plus, FileText, History, Save, RotateCcw, Trash2, ChevronDown, ChevronRight,
} from "lucide-react";
import * as Lucide from "lucide-react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES } from "@/lib/litgraph/types";
import type { LitNode, ChapterVersion } from "@/lib/litgraph/types";

// ====== Внутренний редактор (mount per node id) ======
function EditorBody({ node }: { node: LitNode }) {
  const updateNodeData = useLitStore((s) => s.updateNodeData);
  const updateNodeMeta = useLitStore((s) => s.updateNodeMeta);
  const saveVersion = useLitStore((s) => s.saveVersion);
  const restoreVersion = useLitStore((s) => s.restoreVersion);
  const deleteVersion = useLitStore((s) => s.deleteVersion);
  const allNodes = useLitStore((s) => s.nodes);

  // Инициализируем локальный стейт ОДИН раз при монтировании
  const cfg = NODE_TYPES[node.type];
  const [title, setTitle] = useState(node.data.title);
  const [body, setBody] = useState(node.data.body);
  const [tags, setTags] = useState<string[]>(node.data.tags ?? []);
  const [newTag, setNewTag] = useState("");
  const [fullText, setFullText] = useState<string>(node.data.fullText ?? "");
  const [showVersions, setShowVersions] = useState(false);

  const metaInit: Record<string, string> = {};
  for (const f of cfg.fields) {
    const v = node.data.meta?.[f.key];
    metaInit[f.key] = v === undefined || v === null ? "" : String(v);
  }
  const [meta, setMeta] = useState<Record<string, string>>(metaInit);

  const Icon = (Lucide as any)[cfg.icon] as Lucide.LucideIcon | undefined;
  const Ico = Icon ?? Lucide.Square;

  // Глава и сцена могут иметь полный текст
  const canHaveFullText = node.type === "chapter" || node.type === "scene";
  const wordCount = fullText ? fullText.split(/\s+/).filter(Boolean).length : 0;
  const charCount = fullText.length;

  // Текущие версии ноды (берём из стора, чтобы они обновлялись)
  const currentNode = allNodes.find((n) => n.id === node.id);
  const versions: ChapterVersion[] = currentNode?.data.versions ?? [];

  function commit(patch: Partial<{ title: string; body: string; tags: string[]; fullText: string }>) {
    updateNodeData(node.id, patch);
  }

  function commitMeta(key: string, value: string) {
    setMeta((m) => ({ ...m, [key]: value }));
    const field = cfg.fields.find((f) => f.key === key);
    let val: unknown = value;
    if (field?.type === "number") {
      val = value === "" ? undefined : Number(value);
    }
    updateNodeMeta(node.id, { [key]: val });
  }

  function addTag() {
    const t = newTag.trim().replace(/^#/, "");
    if (!t || tags.includes(t)) return;
    const next = [...tags, t];
    setTags(next);
    setNewTag("");
    commit({ tags: next });
  }

  function removeTag(t: string) {
    const next = tags.filter((x) => x !== t);
    setTags(next);
    commit({ tags: next });
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <div
            className="flex items-center justify-center w-7 h-7 rounded-md"
            style={{ background: cfg.color, color: "#fff" }}
          >
            <Ico className="w-4 h-4" />
          </div>
          <span style={{ color: cfg.color }}>{cfg.singular}</span>
          <span className="text-stone-400 text-sm font-normal">
            · редактирование
          </span>
        </DialogTitle>
      </DialogHeader>

      <div className="flex-1 overflow-y-auto lit-scroll pr-1 space-y-4">
        {/* Заголовок */}
        <div className="space-y-1.5">
          <Label htmlFor="node-title" className="text-xs text-stone-500">
            Название
          </Label>
          <Input
            id="node-title"
            value={title}
            onChange={(e) => {
              setTitle(e.target.value);
              commit({ title: e.target.value });
            }}
            placeholder="Например: «Сцена на вокзале»"
            className="text-base font-medium"
          />
        </div>

        {/* Тело */}
        <div className="space-y-1.5">
          <Label htmlFor="node-body" className="text-xs text-stone-500">
            Содержание / описание
          </Label>
          <Textarea
            id="node-body"
            value={body}
            onChange={(e) => {
              setBody(e.target.value);
              commit({ body: e.target.value });
            }}
            placeholder={cfg.defaultBody}
            className="min-h-[100px] resize-y leading-relaxed"
          />
        </div>

        {/* Полный текст — только для глав и сцен */}
        {canHaveFullText && (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <Label htmlFor="node-fulltext" className="text-xs text-stone-500 flex items-center gap-1.5">
                <FileText className="w-3.5 h-3.5" />
                Полный текст {node.type === "chapter" ? "главы" : "сцены"}
              </Label>
              {fullText && (
                <span className="text-[10px] text-stone-400">
                  {wordCount} сл. · {charCount} симв.
                </span>
              )}
            </div>
            <Textarea
              id="node-fulltext"
              value={fullText}
              onChange={(e) => {
                setFullText(e.target.value);
                commit({ fullText: e.target.value });
              }}
              placeholder="Здесь можно прочитать и редактировать полный текст главы…"
              className="min-h-[300px] resize-y leading-relaxed font-serif text-sm"
              style={{ fontFamily: "Georgia, 'Times New Roman', serif" }}
            />
            <p className="text-[10px] text-stone-400 leading-relaxed">
              Полный текст сохраняется автоматически и виден только в этом окне.
              На холсте в шапке ноды показывается счётчик слов.
            </p>

            {/* Кнопки управления версиями */}
            <div className="flex gap-2 pt-1">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => {
                  saveVersion(node.id, `Сохранено вручную · ${new Date().toLocaleString("ru-RU")}`, "manual");
                }}
                disabled={!fullText.trim()}
                title="Сохранить текущий текст как версию (можно откатиться к ней позже)"
              >
                <Save className="w-3 h-3 mr-1" />
                Сохранить версию
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => setShowVersions(!showVersions)}
              >
                {showVersions ? <ChevronDown className="w-3 h-3 mr-1" /> : <ChevronRight className="w-3 h-3 mr-1" />}
                <History className="w-3 h-3 mr-1" />
                История ({versions.length})
              </Button>
            </div>

            {/* Список версий */}
            {showVersions && (
              <div className="rounded-md border border-stone-200 bg-stone-50 p-2 space-y-1.5 max-h-64 overflow-y-auto lit-scroll">
                {versions.length === 0 ? (
                  <p className="text-xs text-stone-400 text-center py-3">
                    Пока нет сохранённых версий. Нажмите «Сохранить версию».
                  </p>
                ) : (
                  versions.map((v, i) => (
                    <div
                      key={v.id}
                      className="flex items-start gap-2 p-2 rounded bg-white border border-stone-200 hover:border-stone-300"
                    >
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <span className="text-xs font-medium text-stone-700">
                            {i === 0 ? "Последняя" : `#${versions.length - i}`}
                          </span>
                          {v.source && (
                            <Badge
                              variant="secondary"
                              className="text-[9px] h-4"
                              style={{
                                background:
                                  v.source === "ai" ? "#9333EA15" :
                                  v.source === "restore" ? "#CA8A0415" :
                                  v.source === "import" ? "#2563A615" :
                                  "#3D706815",
                                color:
                                  v.source === "ai" ? "#9333EA" :
                                  v.source === "restore" ? "#CA8A04" :
                                  v.source === "import" ? "#2563A6" :
                                  "#3D7068",
                              }}
                            >
                              {v.source}
                            </Badge>
                          )}
                          <span className="text-[10px] text-stone-400">
                            {v.wordCount} сл.
                          </span>
                        </div>
                        <div className="text-[10px] text-stone-500 mt-0.5 truncate">
                          {v.label || new Date(v.timestamp).toLocaleString("ru-RU")}
                        </div>
                        <div className="text-[10px] text-stone-400 mt-0.5">
                          {new Date(v.timestamp).toLocaleString("ru-RU")}
                        </div>
                      </div>
                      <div className="flex flex-col gap-1">
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-6 w-6 p-0"
                          onClick={() => {
                            if (confirm(`Восстановить эту версию? Текущий текст будет сохранён как новая версия, чтобы можно было вернуться.`)) {
                              restoreVersion(node.id, v.id);
                              setFullText(v.fullText);
                            }
                          }}
                          title="Восстановить"
                        >
                          <RotateCcw className="w-3 h-3" />
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-6 w-6 p-0 text-red-500 hover:text-red-700 hover:bg-red-50"
                          onClick={() => {
                            if (confirm("Удалить эту версию безвозвратно?")) {
                              deleteVersion(node.id, v.id);
                            }
                          }}
                          title="Удалить версию"
                        >
                          <Trash2 className="w-3 h-3" />
                        </Button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            )}
          </div>
        )}

        {/* Доп. поля */}
        {cfg.fields.length > 0 && (
          <div className="space-y-3">
            <div className="text-xs text-stone-500 uppercase tracking-wider">
              Детали
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {cfg.fields.map((f) => (
                <div key={f.key} className="space-y-1.5">
                  <Label className="text-xs text-stone-500">{f.label}</Label>
                  {f.type === "textarea" ? (
                    <Textarea
                      value={meta[f.key] ?? ""}
                      onChange={(e) => commitMeta(f.key, e.target.value)}
                      placeholder={f.placeholder}
                      className="min-h-[60px] text-sm"
                    />
                  ) : f.type === "select" ? (
                    <select
                      value={meta[f.key] ?? ""}
                      onChange={(e) => commitMeta(f.key, e.target.value)}
                      className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm"
                    >
                      <option value="">— не указано —</option>
                      {f.options?.map((opt) => (
                        <option key={opt} value={opt}>
                          {opt}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <Input
                      type={f.type === "number" ? "number" : "text"}
                      value={meta[f.key] ?? ""}
                      onChange={(e) => commitMeta(f.key, e.target.value)}
                      placeholder={f.placeholder}
                      className="text-sm"
                    />
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Теги */}
        <div className="space-y-1.5">
          <Label className="text-xs text-stone-500">Теги</Label>
          <div className="flex flex-wrap gap-1.5 min-h-[28px] p-2 rounded-md border border-stone-200 bg-stone-50">
            {tags.length === 0 && (
              <span className="text-xs text-stone-400 self-center">
                пока нет тегов
              </span>
            )}
            {tags.map((t) => (
              <Badge
                key={t}
                variant="secondary"
                className="gap-1"
                style={{ background: `${cfg.color}18`, color: cfg.color }}
              >
                #{t}
                <button
                  onClick={() => removeTag(t)}
                  className="hover:opacity-70"
                  type="button"
                >
                  <X className="w-3 h-3" />
                </button>
              </Badge>
            ))}
          </div>
          <div className="flex gap-2">
            <Input
              value={newTag}
              onChange={(e) => setNewTag(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addTag();
                }
              }}
              placeholder="добавить тег и нажать Enter"
              className="text-sm h-9"
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={addTag}
              className="h-9"
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
        </div>
      </div>

      <DialogFooter className="border-t pt-3">
        <span className="text-xs text-stone-400 self-center hidden sm:inline mr-auto">
          Все изменения сохраняются автоматически
        </span>
      </DialogFooter>
    </>
  );
}

export function NodeEditor() {
  const editingNodeId = useLitStore((s) => s.editingNodeId);
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const node = useLitStore((s) =>
    s.editingNodeId ? s.nodes.find((n) => n.id === s.editingNodeId) ?? null : null
  );

  return (
    <Dialog open={!!editingNodeId} onOpenChange={(o) => !o && setEditingNode(null)}>
      <DialogContent className="max-w-3xl max-h-[92vh] overflow-hidden flex flex-col">
        {node && <EditorBody key={node.id} node={node} />}
        <div className="flex justify-end pt-1">
          <Button variant="outline" onClick={() => setEditingNode(null)}>
            Закрыть
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

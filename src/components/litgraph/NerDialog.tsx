"use client";

import * as Lucide from "lucide-react";
import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { extractEntities } from "@/lib/poler/nerBridge";
import { ENTITY_LABELS, type Entity, type EntityLabel, type NerResult } from "@/lib/poler/nerTypes";

interface NerDialogProps {
  open: boolean;
  text: string;
  onClose: () => void;
}

export function NerDialog({ open, text, onClose }: NerDialogProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<NerResult | null>(null);

  async function handleExtract() {
    if (!text.trim()) {
      setError("Нет текста для анализа. Импортируйте .md сначала.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await extractEntities(text);
      setResult(data);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Группировка по типам сущностей
  const byLabel = useMemo(() => {
    if (!result) return {} as Record<EntityLabel, Entity[]>;
    const groups: Record<EntityLabel, Entity[]> = {
      PER: [], LOC: [], GPE: [], ORG: [], MISC: [],
    };
    for (const e of result.entities) {
      const label = (e.label in groups ? e.label : "MISC") as EntityLabel;
      groups[label].push(e);
    }
    return groups;
  }, [result]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Users className="w-5 h-5 text-amber-600" />
            NER-извлечение: персонажи и локации
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
          {!result && (
            <>
              <p className="text-xs text-stone-500 leading-relaxed">
                <strong>NER (Named Entity Recognition)</strong> — извлечение
                сущностей из текста через <code className="text-amber-700">spaCy</code> +
                <code className="text-amber-700"> pymorphy3</code>. Работает локально,
                без API, без ИИ. Находит:
              </p>
              <ul className="text-xs text-stone-600 space-y-1 ml-4 list-disc">
                <li>
                  <strong style={{ color: ENTITY_LABELS.PER.color }}>PER</strong> — имена
                  людей, фамилии (с объединением падежей: Анна/Анну/Анной → Анна)
                </li>
                <li>
                  <strong style={{ color: ENTITY_LABELS.LOC.color }}>LOC</strong> —
                  географические объекты (Москва, Замок, Сад)
                </li>
                <li>
                  <strong style={{ color: ENTITY_LABELS.ORG.color }}>ORG</strong> —
                  организации, учреждения
                </li>
              </ul>

              <div className="rounded-md bg-amber-50 border border-amber-200 p-3 text-xs text-amber-800">
                <strong>⚠ Требования:</strong>
                <ul className="mt-1 ml-4 list-disc space-y-0.5">
                  <li>Python 3 на машине</li>
                  <li><code>pip install spacy pymorphy3</code></li>
                  <li><code>python -m spacy download ru_core_news_sm</code></li>
                </ul>
                NER работает только в Tauri desktop-версии (не в веб-превью).
              </div>

              <div className="text-[10px] text-stone-400">
                Текст: {text.length.toLocaleString()} символов ·{" "}
                {text.split(/\s+/).filter(Boolean).length.toLocaleString()} слов
                {text.length > 100000 && (
                  <span className="text-amber-600 ml-2">
                    ⚠ будет обрезан до 100k символов
                  </span>
                )}
              </div>
            </>
          )}

          {error && (
            <div className="rounded-md bg-red-50 border border-red-200 p-2.5 text-sm text-red-700">
              ❌ {error}
            </div>
          )}

          {result && (
            <div className="space-y-3">
              {/* Метрики */}
              <div className="grid grid-cols-4 gap-2">
                <MetricBox
                  label="Всего"
                  value={result.stats.total.toString()}
                  color="#1f77b4"
                />
                <MetricBox
                  label="Персонажи"
                  value={result.stats.persons.toString()}
                  color={ENTITY_LABELS.PER.color}
                />
                <MetricBox
                  label="Локации"
                  value={result.stats.locations.toString()}
                  color={ENTITY_LABELS.LOC.color}
                />
                <MetricBox
                  label="Организации"
                  value={result.stats.organizations.toString()}
                  color={ENTITY_LABELS.ORG.color}
                />
              </div>

              {/* Информация о модели */}
              <div className="text-[10px] text-stone-400 flex items-center gap-3">
                <span>Модель: <code>{result.model}</code></span>
                <span>·</span>
                <span>v{result.version}</span>
                {result.truncated && (
                  <>
                    <span>·</span>
                    <span className="text-amber-600">
                      ⚠ текст обрезан {result.processedLength.toLocaleString()}/
                      {result.textLength.toLocaleString()}
                    </span>
                  </>
                )}
              </div>

              {/* Сущности по типам */}
              {(["PER", "LOC", "GPE", "ORG"] as EntityLabel[]).map((label) => {
                const entities = byLabel[label] || [];
                if (entities.length === 0) return null;
                const info = ENTITY_LABELS[label];
                return (
                  <div
                    key={label}
                    className="rounded-md border p-3"
                    style={{ borderColor: info.color + "40", background: info.color + "08" }}
                  >
                    <div className="flex items-center gap-2 mb-2">
                      <Badge
                        style={{ background: info.color, color: "white" }}
                        className="text-xs"
                      >
                        {label}
                      </Badge>
                      <span className="text-sm font-medium">{info.ru}</span>
                      <span className="text-xs text-stone-500">
                        ({entities.length})
                      </span>
                      <span className="text-[10px] text-stone-400 ml-auto">
                        {info.description}
                      </span>
                    </div>

                    <div className="space-y-1.5">
                      {entities.map((e) => (
                        <EntityRow key={e.lemma} entity={e} />
                      ))}
                    </div>
                  </div>
                );
              })}

              {result.stats.total === 0 && (
                <div className="rounded-md bg-stone-50 border border-stone-200 p-4 text-center text-sm text-stone-500">
                  Сущности не найдены. Возможно текст слишком короткий
                  или модель не смогла определить имена.
                </div>
              )}
            </div>
          )}
        </div>

        <DialogFooter className="border-t pt-3">
          {result && (
            <Button
              variant="outline"
              onClick={() => setResult(null)}
              className="mr-auto"
            >
              <Lucide.RefreshCw className="w-4 h-4 mr-1.5" />
              Заново
            </Button>
          )}
          <Button variant="outline" onClick={onClose}>
            Закрыть
          </Button>
          {!result && (
            <Button onClick={handleExtract} disabled={loading}>
              {loading ? (
                <>
                  <Lucide.Loader2 className="w-4 h-4 mr-1.5 animate-spin" />
                  Извлечение…
                </>
              ) : (
                <>
                  <Lucide.Users className="w-4 h-4 mr-1.5" />
                  Запустить NER
                </>
              )}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function EntityRow({ entity }: { entity: Entity }) {
  const [expanded, setExpanded] = useState(false);
  const labelInfo = ENTITY_LABELS[entity.label] || ENTITY_LABELS.MISC;

  return (
    <div
      className="bg-white rounded border-l-2 px-2 py-1.5 cursor-pointer hover:bg-stone-50 transition"
      style={{ borderColor: labelInfo.color }}
      onClick={() => setExpanded(!expanded)}
    >
      <div className="flex items-center gap-2">
        <span
          className="font-medium text-sm"
          style={{ color: labelInfo.color }}
        >
          {entity.lemma}
        </span>
        <Badge variant="outline" className="text-[10px] h-4 px-1">
          ×{entity.count}
        </Badge>
        {entity.forms.length > 1 && (
          <span className="text-[10px] text-stone-500">
            формы: {entity.forms.slice(0, 5).join(", ")}
            {entity.forms.length > 5 && "…"}
          </span>
        )}
        <Lucide.ChevronDown
          className={`w-3 h-3 ml-auto text-stone-400 transition ${
            expanded ? "rotate-180" : ""
          }`}
        />
      </div>
      {expanded && (
        <div className="mt-2 space-y-1 text-[11px] text-stone-600">
          <div className="font-medium text-stone-500">
            Упоминания ({entity.mentions.length}):
          </div>
          {entity.mentions.slice(0, 10).map((m, i) => (
            <div
              key={i}
              className="ml-2 pl-2 border-l-2 border-stone-200 italic"
            >
              "{m.sentence}"
              <span className="text-stone-400 ml-1">
                [pos {m.start}-{m.end}]
              </span>
            </div>
          ))}
          {entity.mentions.length > 10 && (
            <div className="text-stone-400 ml-2">
              … и ещё {entity.mentions.length - 10}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function MetricBox({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color: string;
}) {
  return (
    <div className="rounded-md border p-2 bg-white">
      <div className="text-[10px] text-stone-500">{label}</div>
      <div className="text-lg font-bold" style={{ color }}>
        {value}
      </div>
    </div>
  );
}

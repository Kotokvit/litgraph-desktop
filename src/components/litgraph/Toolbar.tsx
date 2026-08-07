"use client";

import * as Lucide from "lucide-react";
import { useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { callApi } from "@/lib/litgraph/api";
import { Input } from "@/components/ui/input";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  DropdownMenuLabel,
} from "@/components/ui/dropdown-menu";
import { AssistantDialog } from "./AssistantDialog";
import { PolerDialog } from "./PolerDialog";
import { NerDialog } from "./NerDialog";
import { CharacterGraphDialog } from "./CharacterGraphDialog";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useLitStore } from "@/lib/litgraph/store";
import { exportToText, exportToMarkdown, downloadFile, slugify } from "@/lib/litgraph/export";
import { EDGE_TYPES } from "@/lib/litgraph/types";
import type { EdgeKind } from "@/lib/litgraph/types";
// Canvas renderer не использует ReactFlow, fitView через window event
function fitViewViaEvent() {
  window.dispatchEvent(new CustomEvent("litgraph:fitview"));
}
import { AIDialog } from "./AIDialog";

export function Toolbar() {
  const title = useLitStore((s) => s.title);
  const author = useLitStore((s) => s.author);
  const description = useLitStore((s) => s.description);
  const setProjectMeta = useLitStore((s) => s.setProjectMeta);
  const nodesCount = useLitStore((s) => s.nodes.length);
  const edgesCount = useLitStore((s) => s.edges.length);
  const defaultEdgeKind = useLitStore((s) => s.defaultEdgeKind);
  const setDefaultEdgeKind = useLitStore((s) => s.setDefaultEdgeKind);
  const exportProject = useLitStore((s) => s.exportProject);
  const loadProject = useLitStore((s) => s.loadProject);
  const newProject = useLitStore((s) => s.newProject);
  const setNodes = useLitStore((s) => s.setNodes);
  const setEdges = useLitStore((s) => s.setEdges);
  const searchQuery = useLitStore((s) => s.searchQuery);
  const setSearchQuery = useLitStore((s) => s.setSearchQuery);
  const focusEnabled = useLitStore((s) => s.focusEnabled);
  const setFocusEnabled = useLitStore((s) => s.setFocusEnabled);

  const [metaOpen, setMetaOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportText, setExportText] = useState("");
  const [exportFormat, setExportFormat] = useState<"text" | "markdown">("text");
  const [importMdOpen, setImportMdOpen] = useState(false);
  const [mdText, setMdText] = useState("");
  const [mdTitle, setMdTitle] = useState("");
  const [mdAuthor, setMdAuthor] = useState("");
  const [parsing, setParsing] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);
  const [aiMode, setAIMode] = useState<"continue-chapter" | "analyze-plot" | null>(null);
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [polerOpen, setPolerOpen] = useState(false);
  const [nerOpen, setNerOpen] = useState(false);
  const [charGraphOpen, setCharGraphOpen] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const mdFileInputRef = useRef<HTMLInputElement>(null);

  // Собираем текст из всех chapter-нод для POLER-анализа
  const collectedText = useLitStore((s) => {
    const chapters = s.nodes.filter((n) => n.type === "chapter" && n.data.fullText);
    return chapters
      .map((n) => n.data.fullText || "")
      .filter((t) => t.trim().length > 0)
      .join("\n\n");
  });
  

  // ====== Экспорт ======
  function handleExport(format: "text" | "markdown") {
    const proj = exportProject();
    const text = format === "text" ? exportToText(proj) : exportToMarkdown(proj);
    setExportText(text);
    setExportFormat(format);
    setExportOpen(true);
  }

  function handleDownloadExport() {
    const ext = exportFormat === "text" ? "txt" : "md";
    const mime = exportFormat === "text" ? "text/plain" : "text/markdown";
    downloadFile(exportText, `${slugify(title)}.${ext}`, mime);
    setExportOpen(false);
  }

  function handleExportJson() {
    const proj = exportProject();
    const json = JSON.stringify(proj, null, 2);
    downloadFile(json, `${slugify(title)}.litgraph.json`, "application/json");
  }

  // ====== Импорт JSON ======
  function handleImportClick() {
    fileInputRef.current?.click();
  }

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      try {
        const data = JSON.parse(ev.target?.result as string);
        if (!data.nodes || !data.edges) throw new Error("неверный формат");
        loadProject(data);
        setTimeout(() => fitViewViaEvent(), 100);
      } catch (err) {
        alert("Не удалось загрузить файл: " + (err as Error).message);
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  }

  // ====== Импорт .md (автопарсер) ======
  function handleImportMdClick() {
    setMdText("");
    setMdTitle("");
    setMdAuthor(author);
    setParseError(null);
    setImportMdOpen(true);
  }

  function handleMdFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      setMdText(ev.target?.result as string);
      if (!mdTitle) setMdTitle(file.name.replace(/\.md$/i, ""));
    };
    reader.readAsText(file);
    e.target.value = "";
  }

  async function handleParseMd() {
    if (!mdText.trim()) {
      setParseError("Вставьте или загрузите текст");
      return;
    }
    setParsing(true);
    setParseError(null);
    try {
      const data = await callApi("parse_md", "/api/parse-md", {
        markdown: mdText,
        projectTitle: mdTitle || "Импортированный проект",
        author: mdAuthor || "",
      }, "params");
      loadProject(data as any);
      setImportMdOpen(false);
      setTimeout(() => fitViewViaEvent(), 100);
    } catch (err) {
      setParseError(String(err));
    } finally {
      setParsing(false);
    }
  }

  // ====== Управление холстом ======
  function handleAutoLayout() {
    const { nodes, edges } = useLitStore.getState();
    if (nodes.length === 0) return;
    const order = ["chapter", "scene", "plotpoint", "conflict", "character", "dialogue", "location", "idea"];
    const grouped: Record<string, typeof nodes> = {};
    nodes.forEach((n) => {
      const g = grouped[n.type] ?? (grouped[n.type] = []);
      g.push(n);
    });
    const colWidth = 320;
    const rowHeight = 200;
    const newNodes = nodes.map((n) => {
      const col = order.indexOf(n.type);
      const inCol = grouped[n.type].indexOf(n);
      return { ...n, position: { x: 80 + col * colWidth, y: 80 + inCol * rowHeight } };
    });
    setNodes(newNodes);
    void edges;
    setTimeout(() => fitViewViaEvent(), 50);
  }

  function handleFitView() {
    fitViewViaEvent();
  }

  function handleClearAll() {
    if (confirm("Удалить ВСЕ ноды и связи? Это действие необратимо.")) {
      setNodes([]);
      setEdges([]);
    }
  }

  function handleNewProject() {
    if (confirm("Создать новый пустой проект? Текущий будет потерян (если не экспортировали).")) {
      newProject();
    }
  }

  return (
    <>
      <header className="flex items-center gap-2 px-4 py-2 bg-white border-b border-stone-200 shadow-sm flex-wrap">
        {/* Лого / Название */}
        <div className="flex items-center gap-2 mr-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-amber-600 to-stone-700 flex items-center justify-center text-white">
            <Lucide.Feather className="w-4 h-4" />
          </div>
          <div className="hidden sm:block">
            <div className="text-sm font-bold text-stone-800 leading-none">ЛитоГраф</div>
            <div className="text-[10px] text-stone-400 leading-none mt-0.5">нодовый редактор для литературы</div>
          </div>
        </div>

        <div className="h-6 w-px bg-stone-200 mx-1 hidden sm:block" />

        {/* Название проекта */}
        <Input
          value={title}
          onChange={(e) => setProjectMeta({ title: e.target.value })}
          className="h-8 w-40 sm:w-56 text-sm font-medium border-stone-200"
          placeholder="Название проекта"
        />

        {/* Поиск */}
        <div className="relative ml-auto sm:ml-0">
          <Lucide.Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-stone-400" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="поиск по нодам…"
            className="h-8 w-36 sm:w-48 pl-7 text-sm"
          />
        </div>

        {/* Focus toggle */}
        <Button
          variant={focusEnabled ? "default" : "outline"}
          size="sm"
          className="h-8 px-2"
          onClick={() => setFocusEnabled(!focusEnabled)}
          title="Focus-режим: при выборе ноды остальные затемняются"
          style={focusEnabled ? { background: "#8B5A2B" } : undefined}
        >
          <Lucide.Focus className="w-4 h-4" />
          <span className="text-xs ml-1 hidden lg:inline">Focus</span>
        </Button>

        {/* AI */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-8 px-2" style={{ borderColor: "#9333EA40", color: "#9333EA" }}>
              <Lucide.Sparkles className="w-4 h-4" />
              <span className="text-xs ml-1 hidden md:inline">AI</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            <DropdownMenuLabel className="text-xs text-stone-500">
              AI-инструменты
            </DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => setAssistantOpen(true)}>
              <Lucide.MessageCircle className="w-4 h-4 mr-2 text-violet-600" />
              <div className="flex-1">
                <div className="text-sm font-medium">AI-помощник (чат)</div>
                <div className="text-[10px] text-stone-500">Спросить о чём угодно</div>
              </div>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setAIMode("continue-chapter")}>
              <Lucide.PenLine className="w-4 h-4 mr-2 text-amber-600" />
              <div className="flex-1">
                <div className="text-sm font-medium">Дописать главу</div>
                <div className="text-[10px] text-stone-500">На основе последних глав</div>
              </div>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setAIMode("analyze-plot")}>
              <Lucide.AlertTriangle className="w-4 h-4 mr-2 text-rose-600" />
              <div className="flex-1">
                <div className="text-sm font-medium">Анализ сюжета</div>
                <div className="text-[10px] text-stone-500">Найти слабые места</div>
              </div>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Быстрая кнопка помощника */}
        <Button
          variant="default"
          size="sm"
          className="h-8 px-2"
          onClick={() => setAssistantOpen(true)}
          title="AI-помощник (чат)"
          style={{ background: "#9333EA" }}
        >
          <Lucide.MessageCircle className="w-4 h-4" />
          <span className="text-xs ml-1 hidden lg:inline">Спросить AI</span>
        </Button>

        {/* POLER — детерминированный анализ текста (без ИИ) */}
        <Button
          variant="outline"
          size="sm"
          className="h-8 px-2"
          onClick={() => setPolerOpen(true)}
          title="POLER: детерминированный анализ структуры текста"
          style={{ borderColor: "#0EA5E940", color: "#0284C7" }}
        >
          <Lucide.Network className="w-4 h-4" />
          <span className="text-xs ml-1 hidden md:inline">POLER</span>
        </Button>

        {/* NER — извлечение персонажей и локаций (spaCy + pymorphy3) */}
        <Button
          variant="outline"
          size="sm"
          className="h-8 px-2"
          onClick={() => setNerOpen(true)}
          title="NER: извлечение персонажей и локаций (spaCy)"
          style={{ borderColor: "#D9770640", color: "#D97706" }}
        >
          <Lucide.Users className="w-4 h-4" />
          <span className="text-xs ml-1 hidden md:inline">NER</span>
        </Button>

        {/* Граф персонажей — POLER на сущностях (полный текст, без обрезки) */}
        <Button
          variant="outline"
          size="sm"
          className="h-8 px-2"
          onClick={() => setCharGraphOpen(true)}
          title="Граф персонажей: NER + POLER-физика (полный текст)"
          style={{ borderColor: "#7C3AED40", color: "#7C3AED" }}
        >
          <Lucide.Share2 className="w-4 h-4" />
          <span className="text-xs ml-1 hidden md:inline">Граф</span>
        </Button>

        {/* Тип связи по умолчанию */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-8 hidden md:flex">
              <Lucide.GitBranch className="w-3.5 h-3.5 mr-1" />
              Связь:
              <span className="ml-1 font-medium" style={{ color: EDGE_TYPES[defaultEdgeKind].color }}>
                {EDGE_TYPES[defaultEdgeKind].label}
              </span>
              <Lucide.ChevronDown className="w-3 h-3 ml-1" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            <DropdownMenuLabel className="text-xs text-stone-500">Тип новой связи</DropdownMenuLabel>
            <DropdownMenuSeparator />
            {Object.values(EDGE_TYPES).map((k) => (
              <DropdownMenuItem
                key={k.kind}
                onClick={() => setDefaultEdgeKind(k.kind as EdgeKind)}
                className="flex items-start gap-2 py-2"
              >
                <div className="w-2.5 h-2.5 rounded-full mt-1 shrink-0" style={{ background: k.color }} />
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium">{k.label}</div>
                  <div className="text-[10px] text-stone-500 leading-tight">{k.description}</div>
                </div>
                {defaultEdgeKind === k.kind && <Lucide.Check className="w-3.5 h-3.5 text-emerald-600" />}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        <div className="h-6 w-px bg-stone-200 mx-0.5 hidden lg:block" />

        {/* Кнопки управления */}
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={handleFitView} className="h-8 w-8 p-0" title="Уместить в экран">
            <Lucide.Maximize2 className="w-4 h-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={handleAutoLayout}
            className="h-8 px-2 hidden sm:flex"
            title="Авто-раскладка"
          >
            <Lucide.LayoutGrid className="w-4 h-4 mr-1" />
            <span className="text-xs">Авто</span>
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" className="h-8 px-2">
                <Lucide.FileText className="w-4 h-4 mr-1" />
                <span className="text-xs hidden sm:inline">Файл</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-60">
              <DropdownMenuItem onClick={() => setMetaOpen(true)}>
                <Lucide.Settings className="w-4 h-4 mr-2" />О проекте
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleNewProject}>
                <Lucide.FilePlus2 className="w-4 h-4 mr-2" />Новый проект
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleImportMdClick} style={{ background: "#FEF3C7" }}>
                <Lucide.FileCode className="w-4 h-4 mr-2 text-amber-700" />
                <div className="flex-1">
                  <div className="text-sm font-medium text-amber-900">Импорт .md (автопарсер)</div>
                  <div className="text-[10px] text-amber-700">Разобрать любой текст на ноды</div>
                </div>
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleImportClick}>
                <Lucide.Upload className="w-4 h-4 mr-2" />Импорт JSON…
              </DropdownMenuItem>
              <DropdownMenuItem onClick={handleExportJson}>
                <Lucide.Download className="w-4 h-4 mr-2" />Экспорт JSON
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => handleExport("text")}>
                <Lucide.FileText className="w-4 h-4 mr-2" />Экспорт в текст
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => handleExport("markdown")}>
                <Lucide.FileCode className="w-4 h-4 mr-2" />Экспорт в Markdown
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={handleClearAll} className="text-red-600 focus:text-red-700">
                <Lucide.Trash2 className="w-4 h-4 mr-2" />Очистить холст
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {/* Счётчики */}
        <div className="hidden xl:flex items-center gap-3 text-[10px] text-stone-400 ml-2">
          <span>{nodesCount} нод</span>
          <span>·</span>
          <span>{edgesCount} связей</span>
        </div>
      </header>

      <input
        ref={fileInputRef}
        type="file"
        accept=".json,application/json"
        onChange={handleFileChange}
        className="hidden"
      />

      {/* Диалог "О проекте" */}
      <Dialog open={metaOpen} onOpenChange={setMetaOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>О проекте</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label className="text-xs text-stone-500">Название</Label>
              <Input value={title} onChange={(e) => setProjectMeta({ title: e.target.value })} className="text-sm" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-stone-500">Автор</Label>
              <Input value={author} onChange={(e) => setProjectMeta({ author: e.target.value })} className="text-sm" />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-stone-500">Описание</Label>
              <Textarea
                value={description}
                onChange={(e) => setProjectMeta({ description: e.target.value })}
                className="min-h-[80px] text-sm"
                placeholder="Короткое описание произведения, жанр, тема…"
              />
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => setMetaOpen(false)}>Готово</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Диалог экспорта */}
      <Dialog open={exportOpen} onOpenChange={setExportOpen}>
        <DialogContent className="max-w-3xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>Экспорт в {exportFormat === "text" ? "текст" : "Markdown"}</DialogTitle>
          </DialogHeader>
          <Textarea value={exportText} readOnly className="flex-1 min-h-[400px] font-mono text-xs" />
          <DialogFooter>
            <Button variant="outline" onClick={() => navigator.clipboard.writeText(exportText)}>
              <Lucide.Copy className="w-4 h-4 mr-1.5" />Копировать
            </Button>
            <Button onClick={handleDownloadExport}>
              <Lucide.Download className="w-4 h-4 mr-1.5" />Скачать
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Диалог импорта .md */}
      <Dialog open={importMdOpen} onOpenChange={setImportMdOpen}>
        <DialogContent className="max-w-3xl max-h-[90vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Lucide.FileCode className="w-5 h-5 text-amber-600" />
              Импорт .md — автопарсер
            </DialogTitle>
          </DialogHeader>
          <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
            <p className="text-xs text-stone-500 leading-relaxed">
              Загрузите или вставьте текст — система автоматически определит главы
              (по паттернам «Глава N», «Chapter N», «# N»), извлечёт персонажей
              (capitalized слова с частотой 5+) и локации (по предлогам места).
              Полученный граф можно править вручную.
            </p>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Название проекта</Label>
                <Input value={mdTitle} onChange={(e) => setMdTitle(e.target.value)} className="text-sm" placeholder="Мой роман" />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-stone-500">Автор</Label>
                <Input value={mdAuthor} onChange={(e) => setMdAuthor(e.target.value)} className="text-sm" />
              </div>
            </div>
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <Label className="text-xs text-stone-500">Текст произведения (.md)</Label>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 text-xs"
                  onClick={() => mdFileInputRef.current?.click()}
                >
                  <Lucide.Upload className="w-3 h-3 mr-1" />Загрузить файл
                </Button>
                <input
                  ref={mdFileInputRef}
                  type="file"
                  accept=".md,.txt,.markdown,text/*"
                  onChange={handleMdFileChange}
                  className="hidden"
                />
              </div>
              <Textarea
                value={mdText}
                onChange={(e) => setMdText(e.target.value)}
                placeholder="Вставьте сюда текст произведения или загрузите .md файл…"
                className="min-h-[300px] font-mono text-xs"
              />
              <div className="text-[10px] text-stone-400">
                {mdText.length.toLocaleString()} символов · {mdText.split(/\s+/).filter(Boolean).length.toLocaleString()} слов
              </div>
            </div>
            {parseError && (
              <div className="rounded-md bg-red-50 border border-red-200 p-2.5 text-sm text-red-700">
                ❌ {parseError}
              </div>
            )}
          </div>
          <DialogFooter className="border-t pt-3">
            <Button variant="outline" onClick={() => setImportMdOpen(false)} className="mr-auto">
              Отмена
            </Button>
            <Button onClick={handleParseMd} disabled={parsing}>
              {parsing ? (
                <>
                  <Lucide.Loader2 className="w-4 h-4 mr-1.5 animate-spin" />
                  Парсинг…
                </>
              ) : (
                <>
                  <Lucide.Sparkles className="w-4 h-4 mr-1.5" />
                  Разобрать на граф
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* AI Dialog */}
      <AIDialog
        open={aiMode !== null}
        mode={aiMode}
        onClose={() => setAIMode(null)}
      />

      <AssistantDialog
        open={assistantOpen}
        onClose={() => setAssistantOpen(false)}
      />

      <PolerDialog
        open={polerOpen}
        text={collectedText}
        onClose={() => setPolerOpen(false)}
      />

      <NerDialog
        open={nerOpen}
        text={collectedText}
        onClose={() => setNerOpen(false)}
      />

      <CharacterGraphDialog
        open={charGraphOpen}
        text={collectedText}
        onClose={() => setCharGraphOpen(false)}
      />
    </>
  );
}

import { ReactFlowProvider } from "@xyflow/react";
import "@xyflow/react/dist/style.css";

function App() {
  return (
    <ReactFlowProvider>
      <div className="h-screen w-screen flex flex-col overflow-hidden bg-stone-50">
        <header className="flex items-center gap-2 px-4 py-2 bg-white border-b border-stone-200">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-amber-600 to-stone-700 flex items-center justify-center text-white text-sm font-bold">
            L
          </div>
          <h1 className="text-sm font-bold text-stone-800">LitGraph Desktop</h1>
          <span className="text-[10px] text-stone-400 ml-2">v0.1.0 — скелет проекта</span>
        </header>

        <main className="flex-1 flex items-center justify-center">
          <div className="text-center max-w-md px-6">
            <h2 className="text-xl font-bold text-stone-800 mb-2">
              Скелет Tauri-проекта готов
            </h2>
            <p className="text-sm text-stone-600 leading-relaxed mb-4">
              Фронтенд из превью-прототипа нужно перенести сюда из песочницы Z.ai.
              Все 11 компонентов из <code className="bg-stone-100 px-1 rounded">src/components/litgraph/</code> переносятся
              с минимальными правками (заменить <code className="bg-stone-100 px-1 rounded">fetch</code> на <code className="bg-stone-100 px-1 rounded">invoke</code>).
            </p>
            <div className="rounded-md bg-amber-50 border border-amber-200 p-3 text-left text-xs text-amber-800">
              <strong>Следующий шаг:</strong> см. <code>docs/PROMPT_PLAN.md</code> раздел 5.1
              и этап 1 в плане разработки.
            </div>
          </div>
        </main>
      </div>
    </ReactFlowProvider>
  );
}

export default App;

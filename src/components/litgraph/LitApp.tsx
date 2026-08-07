"use client";

import { Toolbar } from "@/components/litgraph/Toolbar";
import { LitCanvas } from "@/components/litgraph/LitCanvas";
import { Sidebar } from "@/components/litgraph/Sidebar";
import { NodeEditor } from "@/components/litgraph/NodeEditor";

export default function LitApp() {
  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden bg-stone-50">
      <Toolbar />
      <div className="flex-1 flex overflow-hidden">
        <LitCanvas />
        <Sidebar />
      </div>
      <NodeEditor />
    </div>
  );
}

import React, { useState } from "react";
import { AppView } from "./lib/types";
import TemplateList from "./components/TemplateList";
import TemplateCreator from "./components/TemplateCreator";
import TemplateFiller from "./components/TemplateFiller";
import { FileText, Plus, List } from "lucide-react";

export default function App() {
  const [view, setView] = useState<AppView>("list");
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);

  const handleCreateTemplate = () => {
    setView("create");
  };

  const handleUseTemplate = (templateId: string) => {
    setSelectedTemplateId(templateId);
    setView("fill");
  };

  const handleBackToList = () => {
    setSelectedTemplateId(null);
    setView("list");
  };

  return (
    <div className="min-h-screen flex flex-col bg-slate-900">
      {/* Header */}
      <header className="bg-slate-800 border-b border-slate-700 px-6 py-4">
        <div className="flex items-center justify-between max-w-7xl mx-auto">
          <div className="flex items-center gap-3">
            <FileText className="w-8 h-8 text-blue-400" />
            <h1 className="text-2xl font-bold text-white">DocForge</h1>
            <span className="text-slate-400 text-sm ml-2">Document Automation</span>
          </div>
          <nav className="flex gap-2">
            <button
              onClick={handleBackToList}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition ${
                view === "list"
                  ? "bg-blue-600 text-white"
                  : "text-slate-300 hover:bg-slate-700"
              }`}
            >
              <List className="w-4 h-4" />
              Templates
            </button>
            <button
              onClick={handleCreateTemplate}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition ${
                view === "create"
                  ? "bg-blue-600 text-white"
                  : "text-slate-300 hover:bg-slate-700"
              }`}
            >
              <Plus className="w-4 h-4" />
              New Template
            </button>
          </nav>
        </div>
      </header>

      {/* Main content */}
      <main className="flex-1 overflow-hidden">
        {view === "list" && (
          <TemplateList onUseTemplate={handleUseTemplate} onCreateTemplate={handleCreateTemplate} />
        )}
        {view === "create" && <TemplateCreator onComplete={handleBackToList} />}
        {view === "fill" && selectedTemplateId && (
          <TemplateFiller templateId={selectedTemplateId} onBack={handleBackToList} />
        )}
      </main>
    </div>
  );
}

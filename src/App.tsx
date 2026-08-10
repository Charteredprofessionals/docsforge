import React, { useState } from "react";
import { AppView } from "./lib/types";
import TemplateList from "./components/TemplateList";
import TemplateCreator from "./components/TemplateCreator";
import TemplateFiller from "./components/TemplateFiller";
import AdminConsole from "./components/AdminConsole";
import Bundles from "./components/Bundles";
import ConsentDialog from "./components/ConsentDialog";
import { FileText, Plus, List, Shield, Settings, Layers } from "lucide-react";

export default function App() {
  const [view, setView] = useState<AppView>("list");
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [showConsentModal, setShowConsentModal] = useState(false);

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
    <div className="min-h-screen flex flex-col bg-slate-900 font-sans antialiased text-slate-100">
      {/* Header */}
      <header className="bg-slate-800/90 border-b border-slate-700/80 px-6 py-3.5 sticky top-0 z-40 backdrop-blur-md">
        <div className="flex items-center justify-between max-w-7xl mx-auto">
          <div className="flex items-center gap-3 cursor-pointer" onClick={handleBackToList}>
            <div className="w-9 h-9 bg-blue-600/20 border border-blue-500/30 rounded-xl flex items-center justify-center">
              <FileText className="w-5 h-5 text-blue-400" />
            </div>
            <div>
              <h1 className="text-xl font-bold text-white tracking-tight leading-none">DocForge</h1>
              <span className="text-slate-400 text-xs font-medium">Document Automation</span>
            </div>
          </div>
          <nav className="flex items-center gap-1.5 bg-slate-900/60 p-1 rounded-xl border border-slate-700/50">
            <button
              onClick={handleBackToList}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "list"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <List className="w-4 h-4" />
              Templates
            </button>
            <button
              onClick={handleCreateTemplate}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "create"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <Plus className="w-4 h-4" />
              New Template
            </button>
            <button
              onClick={() => setView("bundles")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "bundles"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <Layers className="w-4 h-4" />
              Bundles
            </button>
            <button
              onClick={() => setView("admin")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "admin"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <Settings className="w-4 h-4" />
              Admin
            </button>
            <div className="h-4 w-px bg-slate-700 mx-1" />
            <button
              onClick={() => setShowConsentModal(true)}
              className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition"
              title="Privacy & Telemetry Settings"
            >
              <Shield className="w-4 h-4" />
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
        {view === "admin" && <AdminConsole />}
        {view === "bundles" && <Bundles />}
      </main>

      {/* Privacy Consent Modal */}
      {showConsentModal && (
        <ConsentDialog
          onConfirm={(optIn, crashReports) => {
            setShowConsentModal(false);
          }}
        />
      )}
    </div>
  );
}

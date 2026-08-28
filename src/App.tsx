import React, { useEffect, useState } from "react";
import { AppView } from "./lib/types";
import TemplateList from "./components/TemplateList";
import TemplateCreator from "./components/TemplateCreator";
import TemplateFiller from "./components/TemplateFiller";
import AdminConsole from "./components/AdminConsole";
import Bundles from "./components/Bundles";
import BundlesScreen from "./components/BundlesScreen";
import MatterForm from "./components/MatterForm";
import GenerationHistory from "./components/GenerationHistory";
import ConsentDialog from "./components/ConsentDialog";
import { installErrorCapture } from "./lib/errorCapture";
import { setTelemetryConsent, getCurrentUser, CurrentUser } from "./lib/ipc";
import { FileText, Plus, List, Shield, Settings, Layers, LayoutDashboard, FileStack, FolderKanban } from "lucide-react";

export default function App() {
  const [view, setView] = useState<AppView>("dashboard");
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [selectedBundleId, setSelectedBundleId] = useState<string | null>(null);
  const [selectedMatterId, setSelectedMatterId] = useState<string | null>(null);
  const [showConsentModal, setShowConsentModal] = useState(false);
  const [currentUser, setCurrentUser] = useState<CurrentUser | null>(null);

  useEffect(() => {
    installErrorCapture();
    // Establish the current user / RBAC context once on startup so the UI can
    // reflect role-aware state and the backend governance checks have a session.
    getCurrentUser()
      .then(setCurrentUser)
      .catch(() => {
        /* best-effort; backend falls back to the local admin user */
      });
  }, []);

  const handleCreateTemplate = () => {
    setView("create");
  };

  const handleUseTemplate = (templateId: string) => {
    setSelectedTemplateId(templateId);
    setView("fill");
  };

  const handleBackToList = () => {
    setSelectedTemplateId(null);
    setSelectedBundleId(null);
    setSelectedMatterId(null);
    setView("dashboard");
  };

  const handleViewBundle = (bundleId: string) => {
    setSelectedBundleId(bundleId);
    setView("bundle-detail");
  };

  const handleViewMatter = (matterId: string) => {
    setSelectedMatterId(matterId);
    setView("matter-form");
  };

  const handleMatterComplete = () => {
    if (selectedMatterId) {
      setView("generation-history");
    }
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
              onClick={() => setView("dashboard")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "dashboard"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <LayoutDashboard className="w-4 h-4" />
              Dashboard
            </button>
            <button
              onClick={() => setView("bundles")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "bundles" || view === "bundle-detail"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <Layers className="w-4 h-4" />
              Bundles
            </button>
            <button
              onClick={() => setView("matters")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "matters" || view === "matter-form"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <FolderKanban className="w-4 h-4" />
              Matters
            </button>
            <button
              onClick={() => setView("generation-history")}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
                view === "generation-history"
                  ? "bg-blue-600 text-white shadow-md shadow-blue-600/20"
                  : "text-slate-300 hover:text-white hover:bg-slate-800"
              }`}
            >
              <FileStack className="w-4 h-4" />
              Generated Docs
            </button>
            <div className="h-4 w-px bg-slate-700 mx-1" />
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
        {view === "dashboard" && (
          <TemplateList onUseTemplate={handleUseTemplate} onCreateTemplate={handleCreateTemplate} />
        )}
        {view === "list" && (
          <TemplateList onUseTemplate={handleUseTemplate} onCreateTemplate={handleCreateTemplate} />
        )}
        {view === "create" && <TemplateCreator onComplete={handleBackToList} />}
        {view === "fill" && selectedTemplateId && (
          <TemplateFiller templateId={selectedTemplateId} onBack={handleBackToList} />
        )}
        {view === "admin" && <AdminConsole currentUser={currentUser} />}
        {view === "bundles" && <BundlesScreen onViewMatter={handleViewMatter} />}
        {view === "bundle-detail" && selectedBundleId && (
          <BundlesScreen onViewMatter={handleViewMatter} />
        )}
        {view === "matters" && <BundlesScreen onViewMatter={handleViewMatter} />}
        {view === "matter-form" && selectedMatterId && (
          <MatterForm 
            matterId={selectedMatterId}
            onComplete={handleMatterComplete}
            onCancel={() => setView("matters")}
          />
        )}
        {view === "generation-history" && selectedMatterId && (
          <GenerationHistory 
            matterId={selectedMatterId}
            onBack={() => setView("matters")}
          />
        )}
      </main>

      {/* Privacy Consent Modal */}
      {showConsentModal && (
        <ConsentDialog
          onConfirm={async (optIn, crashReports) => {
            try {
              await setTelemetryConsent({ optIn, crashReports });
            } catch (err) {
              console.error("Failed to save telemetry consent:", err);
            }
            setShowConsentModal(false);
          }}
        />
      )}
    </div>
  );
}

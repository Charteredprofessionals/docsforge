import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TemplateMeta } from "../lib/types";
import { Plus, FileText, Play, Trash2, Clock, Tag, Search, AlertTriangle, X } from "lucide-react";

interface Props {
  onUseTemplate: (templateId: string) => void;
  onCreateTemplate: () => void;
}

export default function TemplateList({ onUseTemplate, onCreateTemplate }: Props) {
  const [templates, setTemplates] = useState<TemplateMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [deletingTemplate, setDeletingTemplate] = useState<TemplateMeta | null>(null);

  const loadTemplates = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<string>("list_templates");
      const parsed: TemplateMeta[] = JSON.parse(result);
      setTemplates(parsed);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadTemplates();
  }, []);

  const confirmDelete = async () => {
    if (!deletingTemplate) return;
    try {
      await invoke("delete_template", { templateId: deletingTemplate.id });
      setTemplates((prev) => prev.filter((t) => t.id !== deletingTemplate.id));
      setDeletingTemplate(null);
    } catch (e) {
      setError("Failed to delete template: " + e);
    }
  };

  const getFieldCount = (fieldsJson: string): number => {
    try {
      return JSON.parse(fieldsJson).length;
    } catch {
      return 0;
    }
  };

  const filteredTemplates = templates.filter((t) =>
    t.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="max-w-6xl mx-auto p-6">
      {/* Hero section */}
      <div className="flex flex-col md:flex-row items-center justify-between gap-4 mb-8">
        <div>
          <h2 className="text-3xl font-bold text-white mb-1">Document Templates</h2>
          <p className="text-slate-400 text-sm">
            Create Word templates with fillable fields, then generate completed documents on demand.
          </p>
        </div>

        {templates.length > 0 && (
          <div className="flex items-center gap-3 w-full md:w-auto">
            <div className="relative flex-1 md:w-64">
              <Search className="w-4 h-4 text-slate-500 absolute left-3 top-3" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search templates..."
                className="w-full bg-slate-800 border border-slate-700 rounded-xl pl-9 pr-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500 placeholder-slate-500"
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery("")}
                  className="absolute right-3 top-3 text-slate-500 hover:text-slate-300"
                >
                  <X className="w-4 h-4" />
                </button>
              )}
            </div>
            <button
              onClick={onCreateTemplate}
              className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-xl text-sm font-semibold transition shrink-0 shadow-lg shadow-blue-600/20"
            >
              <Plus className="w-4 h-4" />
              New Template
            </button>
          </div>
        )}
      </div>

      {/* Error display */}
      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-4 py-3 rounded-xl mb-6 flex items-center justify-between text-sm">
          <span>{error}</span>
          <button onClick={() => setError(null)} className="text-red-300 hover:text-white">
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      {/* Loading state */}
      {loading ? (
        <div className="text-center py-20">
          <div className="inline-block w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 mt-4 text-sm">Loading template library...</p>
        </div>
      ) : filteredTemplates.length === 0 ? (
        /* Empty state */
        <div className="text-center py-20 bg-slate-800/40 border border-slate-800 rounded-2xl p-12">
          <FileText className="w-16 h-16 text-slate-600 mx-auto mb-4" />
          <h3 className="text-xl font-semibold text-slate-300 mb-2">
            {searchQuery ? "No matching templates found" : "No templates yet"}
          </h3>
          <p className="text-slate-500 text-sm max-w-md mx-auto mb-6">
            {searchQuery
              ? `No templates match "${searchQuery}". Clear your search query or create a new template.`
              : "Upload a Word document to create your first reusable template with fillable fields."}
          </p>
          <button
            onClick={searchQuery ? () => setSearchQuery("") : onCreateTemplate}
            className="inline-flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-6 py-3 rounded-xl font-semibold transition shadow-lg shadow-blue-600/20"
          >
            {searchQuery ? "Clear Search" : <><Plus className="w-5 h-5" /> Create First Template</>}
          </button>
        </div>
      ) : (
        /* Template grid */
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
          {filteredTemplates.map((template) => {
            const fieldCount = getFieldCount(template.fields_json);
            return (
              <div
                key={template.id}
                onClick={() => onUseTemplate(template.id)}
                className="bg-slate-800/90 border border-slate-700/80 rounded-2xl p-5 cursor-pointer
                         hover:border-blue-500/80 hover:bg-slate-800 transition group flex flex-col justify-between shadow-lg"
              >
                <div>
                  <div className="flex items-start justify-between mb-3">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 bg-blue-600/20 rounded-xl flex items-center justify-center border border-blue-500/20">
                        <FileText className="w-5 h-5 text-blue-400" />
                      </div>
                      <div>
                        <h3 className="text-white font-semibold text-base group-hover:text-blue-300 transition">
                          {template.name}
                        </h3>
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setDeletingTemplate(template);
                      }}
                      className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 transition p-1.5 rounded-lg hover:bg-red-500/10"
                      title="Delete template"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>

                  <div className="flex items-center gap-4 text-xs text-slate-400 mb-5">
                    <span className="flex items-center gap-1.5 bg-slate-900/60 px-2.5 py-1 rounded-lg border border-slate-700/50">
                      <Tag className="w-3.5 h-3.5 text-amber-400" />
                      {fieldCount} field{fieldCount !== 1 ? "s" : ""}
                    </span>
                    <span className="flex items-center gap-1.5">
                      <Clock className="w-3.5 h-3.5 text-slate-500" />
                      {new Date(template.created_at).toLocaleDateString()}
                    </span>
                  </div>
                </div>

                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onUseTemplate(template.id);
                  }}
                  className="w-full flex items-center justify-center gap-2 bg-blue-600/20 text-blue-400 border border-blue-500/30
                           hover:bg-blue-600 hover:text-white py-2 rounded-xl text-sm font-semibold transition"
                >
                  <Play className="w-4 h-4" />
                  Use Template
                </button>
              </div>
            );
          })}
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deletingTemplate && (
        <div className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm flex items-center justify-center p-4 z-50">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl p-6 max-w-sm w-full shadow-2xl">
            <div className="flex items-center gap-3 text-red-400 mb-3">
              <AlertTriangle className="w-6 h-6 shrink-0" />
              <h3 className="text-lg font-bold text-white">Delete Template</h3>
            </div>
            <p className="text-slate-400 text-sm mb-6">
              Are you sure you want to delete <span className="text-white font-semibold">"{deletingTemplate.name}"</span>? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setDeletingTemplate(null)}
                className="px-4 py-2 text-slate-400 hover:text-white text-sm font-medium transition"
              >
                Cancel
              </button>
              <button
                onClick={confirmDelete}
                className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-sm font-semibold rounded-xl transition shadow-lg shadow-red-600/20"
              >
                Delete Template
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

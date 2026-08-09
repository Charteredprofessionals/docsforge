import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TemplateMeta } from "../lib/types";
import { Plus, FileText, Play, Trash2, Clock, Tag } from "lucide-react";

interface Props {
  onUseTemplate: (templateId: string) => void;
  onCreateTemplate: () => void;
}

export default function TemplateList({ onUseTemplate, onCreateTemplate }: Props) {
  const [templates, setTemplates] = useState<TemplateMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm("Delete this template?")) return;
    try {
      await invoke("delete_template", { templateId: id });
      setTemplates((prev) => prev.filter((t) => t.id !== id));
    } catch (e) {
      alert("Failed to delete: " + e);
    }
  };

  const getFieldCount = (fieldsJson: string): number => {
    try {
      return JSON.parse(fieldsJson).length;
    } catch {
      return 0;
    }
  };

  return (
    <div className="max-w-5xl mx-auto p-6">
      {/* Hero section */}
      <div className="text-center mb-8">
        <h2 className="text-3xl font-bold text-white mb-2">Document Templates</h2>
        <p className="text-slate-400">
          Create Word templates with fillable fields, then generate completed documents on demand.
        </p>
      </div>

      {/* Error display */}
      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-4 py-3 rounded-lg mb-6">
          {error}
        </div>
      )}

      {/* Loading state */}
      {loading ? (
        <div className="text-center py-20">
          <div className="inline-block w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full animate-spin" />
          <p className="text-slate-400 mt-4">Loading templates...</p>
        </div>
      ) : templates.length === 0 ? (
        /* Empty state */
        <div className="text-center py-20">
          <FileText className="w-16 h-16 text-slate-600 mx-auto mb-4" />
          <h3 className="text-xl font-semibold text-slate-300 mb-2">No templates yet</h3>
          <p className="text-slate-500 mb-6">
            Upload a Word document to create your first template with fillable fields.
          </p>
          <button
            onClick={onCreateTemplate}
            className="inline-flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-6 py-3 rounded-lg font-medium transition"
          >
            <Plus className="w-5 h-5" />
            Create First Template
          </button>
        </div>
      ) : (
        /* Template grid */
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {templates.map((template) => {
            const fieldCount = getFieldCount(template.fields_json);
            return (
              <div
                key={template.id}
                onClick={() => onUseTemplate(template.id)}
                className="bg-slate-800 border border-slate-700 rounded-xl p-5 cursor-pointer
                         hover:border-blue-500 hover:bg-slate-800/80 transition group"
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 bg-blue-600/20 rounded-lg flex items-center justify-center">
                      <FileText className="w-5 h-5 text-blue-400" />
                    </div>
                    <div>
                      <h3 className="text-white font-semibold">{template.name}</h3>
                    </div>
                  </div>
                  <button
                    onClick={(e) => handleDelete(template.id, e)}
                    className="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 transition p-1"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>

                <div className="flex items-center gap-4 text-sm text-slate-400 mb-4">
                  <span className="flex items-center gap-1">
                    <Tag className="w-3.5 h-3.5" />
                    {fieldCount} field{fieldCount !== 1 ? "s" : ""}
                  </span>
                  <span className="flex items-center gap-1">
                    <Clock className="w-3.5 h-3.5" />
                    {new Date(template.created_at).toLocaleDateString()}
                  </span>
                </div>

                <button
                  onClick={() => onUseTemplate(template.id)}
                  className="w-full flex items-center justify-center gap-2 bg-blue-600/20 text-blue-400
                           hover:bg-blue-600 hover:text-white py-2 rounded-lg text-sm font-medium transition"
                >
                  <Play className="w-4 h-4" />
                  Use Template
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

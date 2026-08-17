import React, { useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { v4 as uuidv4 } from "uuid";
import {
  TemplateField,
  UploadedDocx,
} from "../lib/types";
import {
  docxToHtml,
  base64ToArrayBuffer,
  labelToTagName,
} from "../lib/docxProcessor";
import { Upload, Save, X, Plus, AlertCircle, ArrowLeft, Tag } from "lucide-react";
import FieldModal from "./FieldModal";
import SanitizedPreview from "./SanitizedPreview";

interface Props {
  onComplete: () => void;
}

export default function TemplateCreator({ onComplete }: Props) {
  const [step, setStep] = useState<"upload" | "edit">("upload");
  const [uploadedDocx, setUploadedDocx] = useState<UploadedDocx | null>(null);
  const [htmlPreview, setHtmlPreview] = useState<string>("");
  const [fields, setFields] = useState<TemplateField[]>([]);
  const [templateName, setTemplateName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeSelection, setActiveSelection] = useState<{ text: string; range: Range } | null>(null);

  const previewRef = useRef<HTMLDivElement>(null);

  const handleFileUpload = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Word Documents", extensions: ["docx"] }],
      });

      if (!selected) return;

      const filePath = selected;

      const result = await invoke<string>("upload_docx", {
        filePath,
      });

      const parsed: UploadedDocx = JSON.parse(result);
      setUploadedDocx(parsed);

      if (!parsed.base64) {
        throw new Error("Uploaded document did not return base64 content.");
      }
      const html = await docxToHtml(base64ToArrayBuffer(parsed.base64));
      setHtmlPreview(html);

      const filename =
        filePath.split(/[\\/]/).pop()?.replace(/\.docx$/i, "") || "Template";
      setTemplateName(filename.replace(/[_-]/g, " "));
      setStep("edit");
    } catch (e) {
      setError("Failed to load document: " + e);
    }
  };

  const handleTextSelection = useCallback(() => {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) return;

    const selectedText = selection.toString().trim();
    if (selectedText.length === 0) return;

    const range = selection.getRangeAt(0).cloneRange();
    setActiveSelection({ text: selectedText, range });
  }, []);

  const handleModalSave = (newField: TemplateField) => {
    if (!activeSelection) return;

    setFields((prev) => [...prev, newField]);

    // Highlight the selected text in the preview
    if (previewRef.current && activeSelection.range) {
      try {
        const highlight = document.createElement("span");
        highlight.className = "text-highlight bg-amber-400/20 text-amber-300 font-semibold px-1 rounded border border-amber-400/40";
        highlight.setAttribute("data-field-id", newField.id);
        highlight.title = `Field: ${newField.label} → {{${newField.tagName}}}`;
        activeSelection.range.surroundContents(highlight);
      } catch (e) {
        // Fallback for complex cross-element selection
        console.warn("Could not wrap selection with highlight span (cross-element boundary):", e);
      }
    }

    const selection = window.getSelection();
    selection?.removeAllRanges();
    setActiveSelection(null);
  };

  const removeField = (fieldId: string) => {
    setFields((prev) => prev.filter((f) => f.id !== fieldId));

    // Remove highlight from preview
    if (previewRef.current) {
      const highlight = previewRef.current.querySelector(
        `[data-field-id="${fieldId}"]`
      );
      if (highlight) {
        const parent = highlight.parentNode;
        while (highlight.firstChild) {
          parent?.insertBefore(highlight.firstChild, highlight);
        }
        parent?.removeChild(highlight);
      }
    }
  };

  const handleSave = async () => {
    if (!uploadedDocx) return;
    if (!templateName.trim()) {
      setError("Please enter a template name.");
      return;
    }
    if (fields.length === 0) {
      setError("Please define at least one fillable field.");
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const result = await invoke<string>("save_template", {
        request: {
          name: templateName.trim(),
          original_docx_b64: uploadedDocx.base64,
          fields: fields,
        },
      });

      const parsed = JSON.parse(result);
      if (parsed.success) {
        onComplete();
      }
    } catch (e) {
      setError("Failed to save template: " + e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="bg-slate-800 border-b border-slate-700 px-6 py-3 flex items-center gap-4">
        <button
          onClick={step === "edit" ? () => setStep("upload") : onComplete}
          className="flex items-center gap-2 text-slate-400 hover:text-white transition text-sm"
        >
          <ArrowLeft className="w-4 h-4" />
          Back
        </button>
        <div className="h-5 w-px bg-slate-600" />
        <h2 className="text-white font-semibold">
          {step === "upload" ? "Upload Document" : "Create Template"}
        </h2>
        {step === "edit" && (
          <div className="ml-auto flex items-center gap-3">
            <input
              type="text"
              value={templateName}
              onChange={(e) => setTemplateName(e.target.value)}
              placeholder="Template name..."
              className="bg-slate-700 border border-slate-600 text-white px-3 py-1.5 rounded-lg text-sm
                       focus:outline-none focus:border-blue-500 w-64"
            />
            <button
              onClick={handleSave}
              disabled={saving || fields.length === 0}
              className="flex items-center gap-2 bg-green-600 hover:bg-green-500 disabled:bg-slate-600
                       text-white px-4 py-1.5 rounded-lg text-sm font-medium transition"
            >
              <Save className="w-4 h-4" />
              {saving ? "Saving..." : "Save Template"}
            </button>
          </div>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="bg-red-900/50 border-b border-red-700 text-red-200 px-6 py-3 flex items-center gap-2 text-sm">
          <AlertCircle className="w-4 h-4" />
          {error}
        </div>
      )}

      {/* Upload step */}
      {step === "upload" && (
        <div className="flex-1 flex items-center justify-center p-6">
          <div className="text-center max-w-md">
            <div
              onClick={handleFileUpload}
              className="border-2 border-dashed border-slate-600 rounded-2xl p-12 cursor-pointer
                       hover:border-blue-500 hover:bg-blue-500/5 transition group"
            >
              <Upload className="w-16 h-16 text-slate-500 group-hover:text-blue-400 mx-auto mb-4 transition" />
              <h3 className="text-xl font-semibold text-white mb-2">
                Upload a Word Document
              </h3>
              <p className="text-slate-400 text-sm">
                Select a .docx file to use as the base for your template.
                You'll be able to highlight text and assign fillable field labels.
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Edit step */}
      {step === "edit" && (
        <div className="flex-1 flex overflow-hidden">
          {/* Preview pane */}
          <div className="flex-1 overflow-auto p-6">
            <div className="mb-4">
              <p className="text-slate-400 text-sm">
                Select text in the document to create a fillable field. Each selected
                area will become a placeholder that users can fill in.
              </p>
            </div>
            <SanitizedPreview
              ref={previewRef}
              html={htmlPreview}
              onTextSelection={handleTextSelection}
            />
          </div>

          {/* Fields sidebar */}
          <div className="w-80 bg-slate-800 border-l border-slate-700 flex flex-col">
            <div className="p-4 border-b border-slate-700">
              <div className="flex items-center justify-between">
                <h3 className="text-white font-semibold">
                  Fields ({fields.length})
                </h3>
              </div>
              <p className="text-slate-400 text-xs mt-1">
                Select text in the preview to add fields.
              </p>
            </div>

            <div className="flex-1 overflow-auto p-4 space-y-2">
              {fields.length === 0 ? (
                <div className="text-center py-8">
                  <Plus className="w-8 h-8 text-slate-600 mx-auto mb-2" />
                  <p className="text-slate-500 text-sm">
                    No fields defined yet.
                    <br />
                    Select text in the document to begin.
                  </p>
                </div>
              ) : (
                fields.map((field, index) => (
                  <div
                    key={field.id}
                    className="bg-slate-700/50 border border-slate-600 rounded-xl p-3 group hover:border-slate-500 transition"
                  >
                    <div className="flex items-start justify-between mb-1">
                      <div className="flex items-center gap-1.5">
                        <span className="text-xs text-blue-400 font-mono">
                          #{index + 1}
                        </span>
                        {field.fieldType && field.fieldType !== "text" && (
                          <span className="text-[10px] uppercase font-bold tracking-wider text-slate-300 bg-slate-800 px-1.5 py-0.5 rounded border border-slate-600">
                            {field.fieldType}
                          </span>
                        )}
                        {field.required && (
                          <span className="text-red-400 text-xs font-bold" title="Required field">*</span>
                        )}
                      </div>
                      <button
                        onClick={() => removeField(field.id)}
                        className="text-slate-500 hover:text-red-400 transition opacity-0 group-hover:opacity-100 p-1"
                        title="Remove field"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                    <div className="text-white text-sm font-medium mb-1">
                      {field.label}
                    </div>
                    <div className="text-slate-400 text-xs truncate mb-1.5" title={field.originalText}>
                      "{field.originalText}"
                    </div>
                    <div className="text-xs font-mono text-amber-400 bg-amber-400/10 px-2 py-0.5 rounded inline-block">
                      {`{{${field.tagName}}}`}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      {/* Field Creation Modal */}
      {activeSelection && (
        <FieldModal
          selectedText={activeSelection.text}
          existingTags={fields.map((f) => f.tagName)}
          onSave={handleModalSave}
          onCancel={() => setActiveSelection(null)}
        />
      )}
    </div>
  );
}

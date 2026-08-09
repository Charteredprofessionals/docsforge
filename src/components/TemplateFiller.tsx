import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TemplateFull } from "../lib/types";
import {
  docxToHtml,
  base64ToArrayBuffer,
  downloadBase64File,
} from "../lib/docxProcessor";
import {
  ArrowLeft,
  FileText,
  Download,
  FileDown,
  Loader2,
  AlertCircle,
  Eye,
  PenLine,
} from "lucide-react";

interface Props {
  templateId: string;
  onBack: () => void;
}

type FillStep = "form" | "preview";

export default function TemplateFiller({ templateId, onBack }: Props) {
  const [template, setTemplate] = useState<TemplateFull | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [step, setStep] = useState<FillStep>("form");
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  const [previewHtml, setPreviewHtml] = useState<string>("");
  const [exportingPdf, setExportingPdf] = useState(false);

  useEffect(() => {
    loadTemplate();
  }, [templateId]);

  const fillTemplate = async (): Promise<string> => {
    const result = await invoke<string>("fill_template", {
      request: {
        templateId: templateId,
        values: fieldValues,
      },
    });
    const parsed = JSON.parse(result);
    return parsed.docx_base64 as string;
  };

  const loadTemplate = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<string>("get_template", {
        templateId,
      });
      const parsed: TemplateFull = JSON.parse(result);
      setTemplate(parsed);

      // Initialize empty values
      const values: Record<string, string> = {};
      parsed.fields.forEach((f) => {
        values[f.tagName] = "";
      });
      setFieldValues(values);
    } catch (e) {
      setError("Failed to load template: " + e);
    } finally {
      setLoading(false);
    }
  };

  const handlePreview = async () => {
    if (!template) return;

    try {
      const filledB64 = await fillTemplate();
      const buffer = base64ToArrayBuffer(filledB64);
      const html = await docxToHtml(buffer);
      setPreviewHtml(html);
      setStep("preview");
    } catch (e) {
      setError("Failed to generate preview: " + e);
    }
  };

  const handleExportWord = async () => {
    if (!template) return;

    try {
      const filledB64 = await fillTemplate();
      const filename = `${template.name}_filled.docx`;
      downloadBase64File(filledB64, filename);
    } catch (e) {
      setError("Failed to export Word: " + e);
    }
  };

  const handleExportPdf = async () => {
    if (!template) return;

    setExportingPdf(true);
    setError(null);

    try {
      const filledB64 = await fillTemplate();

      const result = await invoke<string>("export_to_pdf", {
        request: {
          docx_base64: filledB64,
          output_filename: `${template.name}_filled`,
        },
      });

      const parsed = JSON.parse(result);
      downloadBase64File(parsed.pdf_base64, parsed.filename);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("LibreOffice not found")) {
        setError(
          "PDF export requires LibreOffice installed and available on PATH. " +
            "Please install LibreOffice or export as Word instead."
        );
      } else {
        setError("Failed to export PDF: " + e);
      }
    } finally {
      setExportingPdf(false);
    }
  };

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <Loader2 className="w-10 h-10 text-blue-400 animate-spin mx-auto mb-4" />
          <p className="text-slate-400">Loading template...</p>
        </div>
      </div>
    );
  }

  if (!template) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <AlertCircle className="w-10 h-10 text-red-400 mx-auto mb-4" />
          <p className="text-slate-400 mb-4">{error || "Template not found"}</p>
          <button
            onClick={onBack}
            className="text-blue-400 hover:text-blue-300 text-sm"
          >
            Back to templates
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="bg-slate-800 border-b border-slate-700 px-6 py-3 flex items-center gap-4">
        <button
          onClick={step === "preview" ? () => setStep("form") : onBack}
          className="flex items-center gap-2 text-slate-400 hover:text-white transition text-sm"
        >
          <ArrowLeft className="w-4 h-4" />
          {step === "preview" ? "Edit Values" : "Templates"}
        </button>
        <div className="h-5 w-px bg-slate-600" />
        <div className="flex items-center gap-2">
          <FileText className="w-5 h-5 text-blue-400" />
          <h2 className="text-white font-semibold">{template.name}</h2>
        </div>

        <div className="ml-auto flex items-center gap-2">
          {step === "form" ? (
            <button
              onClick={handlePreview}
              className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white
                       px-4 py-1.5 rounded-lg text-sm font-medium transition"
            >
              <Eye className="w-4 h-4" />
              Preview Document
            </button>
          ) : (
            <>
              <button
                onClick={handleExportWord}
                className="flex items-center gap-2 bg-green-600 hover:bg-green-500 text-white
                         px-4 py-1.5 rounded-lg text-sm font-medium transition"
              >
                <Download className="w-4 h-4" />
                Export Word
              </button>
              <button
                onClick={handleExportPdf}
                disabled={exportingPdf}
                className="flex items-center gap-2 bg-red-600 hover:bg-red-500 disabled:bg-slate-600
                         text-white px-4 py-1.5 rounded-lg text-sm font-medium transition"
              >
                {exportingPdf ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <FileDown className="w-4 h-4" />
                )}
                {exportingPdf ? "Converting..." : "Export PDF"}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-red-900/50 border-b border-red-700 text-red-200 px-6 py-3 flex items-center gap-2 text-sm">
          <AlertCircle className="w-4 h-4" />
          {error}
          <button
            onClick={() => setError(null)}
            className="ml-auto text-red-300 hover:text-white"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Form step */}
      {step === "form" && (
        <div className="flex-1 flex overflow-hidden">
          {/* Form panel */}
          <div className="flex-1 overflow-auto p-6">
            <div className="max-w-2xl mx-auto">
              <div className="mb-6">
                <h3 className="text-xl font-semibold text-white mb-2">
                  Fill Template Fields
                </h3>
                <p className="text-slate-400 text-sm">
                  Enter values for each field. Click "Preview Document" to see the
                  result, then export as Word or PDF.
                </p>
              </div>

              <div className="space-y-4">
                {template.fields.map((field, index) => (
                  <div key={field.id} className="bg-slate-800 border border-slate-700 rounded-lg p-4">
                    <div className="flex items-center justify-between mb-2">
                      <label className="text-white font-medium text-sm">
                        {field.label}
                      </label>
                      <span className="text-xs font-mono text-amber-400 bg-amber-400/10 px-2 py-0.5 rounded">
                        {`{{${field.tagName}}}`}
                      </span>
                    </div>
                    <div className="text-slate-500 text-xs mb-2">
                      Original text: "{field.originalText}"
                    </div>
                    <input
                      type="text"
                      value={fieldValues[field.tagName] || ""}
                      onChange={(e) =>
                        setFieldValues((prev) => ({
                          ...prev,
                          [field.tagName]: e.target.value,
                        }))
                      }
                      placeholder={`Enter value for ${field.label}...`}
                      className="w-full bg-slate-700 border border-slate-600 text-white px-3 py-2 rounded-lg
                               text-sm focus:outline-none focus:border-blue-500 placeholder-slate-500"
                    />
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Info sidebar */}
          <div className="w-72 bg-slate-800 border-l border-slate-700 p-4">
            <h4 className="text-white font-semibold mb-3 flex items-center gap-2">
              <PenLine className="w-4 h-4 text-blue-400" />
              Template Info
            </h4>
            <div className="space-y-3 text-sm">
              <div>
                <span className="text-slate-400">Template:</span>
                <span className="text-white ml-2">{template.name}</span>
              </div>
              <div>
                <span className="text-slate-400">Fields:</span>
                <span className="text-white ml-2">{template.fields.length}</span>
              </div>
              <div>
                <span className="text-slate-400">Created:</span>
                <span className="text-white ml-2">
                  {new Date(template.created_at).toLocaleDateString()}
                </span>
              </div>
              <div className="pt-3 border-t border-slate-700">
                <span className="text-slate-400 text-xs">
                  Fill in all fields, then click "Preview Document" to see the
                  completed document before exporting.
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Preview step */}
      {step === "preview" && (
        <div className="flex-1 overflow-auto p-6">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <h3 className="text-white font-semibold">Document Preview</h3>
              <p className="text-slate-400 text-sm">
                Review the filled document before exporting.
              </p>
            </div>
          </div>
          <div
            className="docx-preview"
            dangerouslySetInnerHTML={{ __html: previewHtml }}
          />
        </div>
      )}
    </div>
  );
}

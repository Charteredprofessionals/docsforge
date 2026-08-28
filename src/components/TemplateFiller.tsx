import React, { useEffect, useState, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { TemplateFull } from "../lib/types";
import {
  docxToHtml,
  base64ToArrayBuffer,
  downloadBase64File,
} from "../lib/docxProcessor";
import { exportTemplateFieldsCsv, batchFillFromCsv, BatchFillResult, getTemplate, fillTemplate, exportToPdf } from "../lib/ipc";
import SanitizedPreview from "./SanitizedPreview";
import {
  ArrowLeft,
  FileText,
  Download,
  FileDown,
  Loader2,
  AlertCircle,
  Eye,
  PenLine,
  CheckCircle2,
  FileSpreadsheet,
  Upload,
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
  const [toast, setToast] = useState<string | null>(null);
  const [replaceAll, setReplaceAll] = useState(true);

  // ── Mail-merge: CSV batch generation for THIS template ──────────────────────
  const [showBatch, setShowBatch] = useState(false);
  const [batchCsvContent, setBatchCsvContent] = useState("");
  const [batchCsvName, setBatchCsvName] = useState("");
  const [batchOutputDir, setBatchOutputDir] = useState("");
  const [batchFormats, setBatchFormats] = useState<{ docx: boolean; pdf: boolean }>({
    docx: true,
    pdf: false,
  });
  const [batchGenerating, setBatchGenerating] = useState(false);
  const [batchResult, setBatchResult] = useState<BatchFillResult | null>(null);

  // FIX #5: Cache the filled docx base64. Invalidate whenever field values change
  // so we never make 3 redundant IPC round-trips for preview + word + pdf.
  const [cachedFilledB64, setCachedFilledB64] = useState<string | null>(null);

  const showToast = (message: string) => {
    setToast(message);
    setTimeout(() => setToast(null), 4000);
  };

  useEffect(() => {
    loadTemplate();
  }, [templateId]);

  // Invalidate cache whenever the user changes any field value or the replaceAll toggle
  useEffect(() => {
    setCachedFilledB64(null);
  }, [fieldValues, replaceAll]);

  // FIX #11: Pre-flight required field validation
  const validateRequiredFields = useCallback((): boolean => {
    if (!template) return true;
    const missing = template.fields
      .filter((f) => f.required && !fieldValues[f.tagName]?.trim())
      .map((f) => f.label);
    if (missing.length > 0) {
      setError(`Required fields are missing: ${missing.join(", ")}`);
      return false;
    }
    return true;
  }, [template, fieldValues]);

  // FIX #5: Single fill call — returns cached result if values haven't changed
  const getFilledDocx = useCallback(async (): Promise<string> => {
    if (cachedFilledB64) return cachedFilledB64;
    const b64 = await fillTemplate({
      templateId,
      values: fieldValues,
      replaceAll,
    });
    setCachedFilledB64(b64);
    return b64;
  }, [cachedFilledB64, templateId, fieldValues, replaceAll]);

  const loadTemplate = async () => {
    setLoading(true);
    setError(null);
    try {
      const parsed: TemplateFull = await getTemplate(templateId);
      setTemplate(parsed);
      const values: Record<string, string> = {};
      parsed.fields.forEach((f) => {
        values[f.tagName] = "";
      });
      setFieldValues(values);
    } catch (e) {
      setError("Failed to load template: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setLoading(false);
    }
  };

  const handlePreview = async () => {
    if (!template) return;
    if (!validateRequiredFields()) return;
    try {
      const filledB64 = await getFilledDocx();
      const buffer = base64ToArrayBuffer(filledB64);
      const html = await docxToHtml(buffer);
      setPreviewHtml(html);
      setStep("preview");
    } catch (e) {
      setError("Failed to generate preview: " + (e instanceof Error ? e.message : String(e)));
    }
  };

  const handleExportWord = async () => {
    if (!template) return;
    if (!validateRequiredFields()) return;
    try {
      const filledB64 = await getFilledDocx();
      const filename = `${template.name}_filled.docx`;
      downloadBase64File(filledB64, filename);
      showToast(`Exported ${filename} successfully!`);
    } catch (e) {
      setError("Failed to export Word: " + (e instanceof Error ? e.message : String(e)));
    }
  };

  const handleExportPdf = async () => {
    if (!template) return;
    if (!validateRequiredFields()) return;
    setExportingPdf(true);
    setError(null);
    try {
      const filledB64 = await getFilledDocx();
      const result = await exportToPdf(filledB64, `${template.name}_filled`);
      downloadBase64File(result.pdfBase64, result.filename);
      const engineNote = result.engine === "native" 
        ? " (native engine - plain text layout)" 
        : "";
      showToast(`Exported ${result.filename} successfully${engineNote}!`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("Native PDF engine failed")) {
        setError(
          "PDF export failed. The native engine encountered an error. Install LibreOffice for fallback support."
        );
      } else {
        setError("Failed to export PDF: " + msg);
      }
    } finally {
      setExportingPdf(false);
    }
  };

  // ── Mail-merge: download this template's fields as a CSV (one column per {{field}}) ──
  const handleExportFieldsCsv = async () => {
    if (!template) return;
    try {
      const csv = await exportTemplateFieldsCsv(template.id);
      const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${template.name}_fields.csv`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(`Failed to export fields CSV: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  // ── Mail-merge: read an uploaded CSV file into memory ──────────────────────
  const handleBatchFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    setBatchCsvContent(text);
    setBatchCsvName(file.name);
    setBatchResult(null);
  };

  const handleBatchOutputDir = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === "string") {
      setBatchOutputDir(selected);
    }
  };

  // ── Mail-merge: fill this template once per CSV row, write to output folder ─
  const handleBatchGenerate = async () => {
    if (!template) return;
    if (!batchCsvContent.trim() || !batchOutputDir.trim()) {
      setError("Select a CSV file and an output directory before generating.");
      return;
    }
    setBatchGenerating(true);
    setError(null);
    try {
      const formats: string[] = [] as string[];
      if (batchFormats.docx) formats.push("docx");
      if (batchFormats.pdf) formats.push("pdf");
      const res = await batchFillFromCsv({
        templateId: template.id,
        csv: batchCsvContent,
        outputDir: batchOutputDir,
        formats,
      });
      setBatchResult(res);
    } catch (e) {
      setError(`Batch generation failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBatchGenerating(false);
    }
  };

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="bg-slate-800 border-b border-slate-700 px-6 py-3 flex items-center gap-4">
        <button
          onClick={onBack}
          className="flex items-center gap-2 text-slate-400 hover:text-white transition text-sm"
        >
          <ArrowLeft className="w-4 h-4" />
          Back
        </button>
        <div className="h-5 w-px bg-slate-600" />
        <h2 className="text-white font-semibold truncate">
          {template ? template.name : "Fill Template"}
        </h2>
      </div>

      {/* Error bar */}
      {error && (
        <div className="bg-red-900/50 border-b border-red-700 text-red-200 px-6 py-3 flex items-center gap-2 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
        </div>
      )}

      {/* Body */}
      <div className="flex-1 overflow-hidden flex">
        {/* Field form */}
        <div className="flex-1 overflow-auto p-6">
          {step === "form" ? (
            <div className="max-w-2xl mx-auto">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 bg-blue-600/20 rounded-xl flex items-center justify-center border border-blue-500/30">
                  <FileText className="w-5 h-5 text-blue-400" />
                </div>
                <div>
                  <h3 className="text-white font-semibold text-lg">
                    Fill Template Fields
                  </h3>
                  <p className="text-slate-400 text-xs">
                    Complete each field, then preview or export the finished document.
                  </p>
                </div>
              </div>

              <div className="space-y-4">
                {template?.fields.map((field, idx) => (
                  <div key={field.id} className="bg-slate-800/60 border border-slate-700/60 rounded-xl p-4">
                    <label className="block text-slate-200 text-sm font-medium mb-2">
                      <span className="text-blue-400 font-mono mr-2">#{idx + 1}</span>
                      {field.label}
                      {field.required && (
                        <span className="text-red-400 ml-1" title="Required">
                          *
                        </span>
                      )}
                    </label>
                    {field.fieldType === "dropdown" ? (
                      <select
                        value={fieldValues[field.tagName] ?? ""}
                        onChange={(e) =>
                          setFieldValues((v) => ({ ...v, [field.tagName]: e.target.value }))
                        }
                        className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-blue-500"
                      >
                        <option value="">— select —</option>
                        {(field.options ?? []).map((opt) => (
                          <option key={opt} value={opt}>
                            {opt}
                          </option>
                        ))}
                      </select>
                    ) : field.fieldType === "checkbox" ? (
                      <input
                        type="checkbox"
                        checked={fieldValues[field.tagName] === "true"}
                        onChange={(e) =>
                          setFieldValues((v) => ({
                            ...v,
                            [field.tagName]: e.target.checked ? "true" : "false",
                          }))
                        }
                        className="w-5 h-5 accent-blue-500"
                      />
                    ) : (
                      <input
                        type={field.fieldType === "date" ? "date" : "text"}
                        value={fieldValues[field.tagName] ?? ""}
                        onChange={(e) =>
                          setFieldValues((v) => ({ ...v, [field.tagName]: e.target.value }))
                        }
                        className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-blue-500"
                        placeholder={field.originalText ? `e.g. ${field.originalText}` : ""}
                      />
                    )}
                    <p className="text-slate-500 text-xs mt-1 font-mono">
                      {"{{" + field.tagName + "}}"}
                    </p>
                  </div>
                ))}
              </div>

              {/* Replace-all toggle */}
              <div className="mt-6 bg-slate-800/40 border border-slate-700/60 rounded-xl p-4 flex items-start gap-3">
                <input
                  id="replace-all"
                  type="checkbox"
                  checked={replaceAll}
                  onChange={(e) => setReplaceAll(e.target.checked)}
                  className="mt-0.5 w-4 h-4 rounded bg-slate-800 border-slate-700 text-blue-600 focus:ring-blue-500"
                />
                <div>
                  <label htmlFor="replace-all" className="text-slate-200 text-sm font-medium cursor-pointer">
                    Replace all occurrences of each placeholder
                  </label>
                  <p className="text-slate-400 text-xs mt-1">
                    When on, every instance of a <span className="font-mono">{"{{field}}"}</span> is
                    filled. Turn off to replace only the first occurrence (useful for repeating labels).
                  </p>
                </div>
              </div>

              <div className="flex flex-wrap gap-3 mt-6">
                <button
                  onClick={handlePreview}
                  className="flex items-center gap-2 bg-slate-700 hover:bg-slate-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition"
                >
                  <Eye className="w-4 h-4" /> Preview
                </button>
                <button
                  onClick={handleExportWord}
                  className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg text-sm font-medium transition shadow-lg shadow-blue-600/20"
                >
                  <Download className="w-4 h-4" /> Export Word
                </button>
                <button
                  onClick={handleExportPdf}
                  disabled={exportingPdf}
                  className="flex items-center gap-2 bg-green-600 hover:bg-green-500 disabled:bg-slate-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition"
                >
                  {exportingPdf ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <FileDown className="w-4 h-4" />
                  )}
                  Export PDF
                </button>
              </div>

              {/* Mail-merge panel: CSV download + batch generate for THIS template */}
              <div className="mt-6 bg-slate-800/40 border border-slate-700/60 rounded-xl p-4">
                <div className="flex items-center justify-between">
                  <h4 className="text-slate-200 text-sm font-semibold flex items-center gap-2">
                    <FileSpreadsheet className="w-4 h-4 text-blue-400" /> Mail Merge — CSV Batch
                  </h4>
                  <button
                    onClick={() => setShowBatch((s) => !s)}
                    className="text-xs text-slate-400 hover:text-white transition"
                  >
                    {showBatch ? "Hide" : "Show"}
                  </button>
                </div>

                <div className="flex flex-wrap gap-3 mt-3">
                  <button
                    onClick={handleExportFieldsCsv}
                    className="flex items-center gap-2 bg-slate-700 hover:bg-slate-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition"
                  >
                    <Download className="w-4 h-4" /> Export Fields CSV
                  </button>
                  <button
                    onClick={() => setShowBatch(true)}
                    className="flex items-center gap-2 bg-purple-600 hover:bg-purple-500 text-white px-4 py-2 rounded-lg text-sm font-medium transition"
                  >
                    <Upload className="w-4 h-4" /> Batch Generate from CSV
                  </button>
                </div>

                {showBatch && (
                  <div className="mt-4 space-y-3 border-t border-slate-700/60 pt-4">
                    <p className="text-slate-400 text-xs">
                      Each CSV row produces one <span className="font-mono">.docx</span>. Columns match this
                      template's <span className="font-mono">{"{{fields}}"}</span>; an{" "}
                      <span className="font-mono">output_filename</span> column names each file.
                    </p>

                    <div>
                      <label className="block text-slate-300 text-xs mb-1">
                        CSV File {batchCsvName && <span className="text-slate-500">({batchCsvName})</span>}
                      </label>
                      <input
                        type="file"
                        accept=".csv"
                        onChange={handleBatchFileChange}
                        className="w-full bg-slate-900 border border-slate-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500"
                      />
                    </div>

                    <div className="flex items-center gap-3">
                      <button
                        onClick={handleBatchOutputDir}
                        className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 text-white px-3 py-2 rounded-lg text-sm transition"
                      >
                        <Upload className="w-4 h-4" /> Output Folder
                      </button>
                      <span className="text-slate-400 text-xs truncate max-w-xs">
                        {batchOutputDir || "No folder selected"}
                      </span>
                    </div>

                    <div className="flex items-center gap-4">
                      <label className="flex items-center gap-2 text-slate-300 text-sm">
                        <input
                          type="checkbox"
                          checked={batchFormats.docx}
                          onChange={(e) => setBatchFormats((f) => ({ ...f, docx: e.target.checked }))}
                          className="accent-blue-500"
                        />
                        DOCX
                      </label>
                      <label className="flex items-center gap-2 text-slate-300 text-sm">
                        <input
                          type="checkbox"
                          checked={batchFormats.pdf}
                          onChange={(e) => setBatchFormats((f) => ({ ...f, pdf: e.target.checked }))}
                          className="accent-blue-500"
                        />
                        PDF
                      </label>
                    </div>

                    <button
                      onClick={handleBatchGenerate}
                      disabled={batchGenerating || !batchCsvContent || !batchOutputDir}
                      className="w-full flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 disabled:bg-slate-600 text-white px-4 py-2 rounded-lg text-sm font-medium transition"
                    >
                      {batchGenerating ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <FileSpreadsheet className="w-4 h-4" />
                      )}
                      Generate Documents
                    </button>

                    {batchResult && (
                      <div className="p-3 bg-slate-800/50 rounded border border-slate-700 text-slate-300 text-xs space-y-1">
                        <div>
                          Generated <span className="text-green-400 font-semibold">{batchResult.generated.length}</span>{" "}
                          document(s).
                        </div>
                        {batchResult.errors.length > 0 && (
                          <div className="text-red-400">
                            {batchResult.errors.length} error(s): {batchResult.errors[0]}
                          </div>
                        )}
                        {batchResult.warnings.length > 0 && (
                          <div className="text-amber-400">{batchResult.warnings.length} warning(s).</div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="w-full">
              <div className="flex items-center justify-between mb-4 max-w-4xl mx-auto">
                <h3 className="text-white font-semibold flex items-center gap-2">
                  <PenLine className="w-4 h-4 text-blue-400" /> Document Preview
                </h3>
                <button
                  onClick={() => setStep("form")}
                  className="text-slate-400 hover:text-white text-sm transition"
                >
                  ← Back to editing
                </button>
              </div>
              {/* FIX #6: Use SanitizedPreview instead of dangerouslySetInnerHTML */}
              <SanitizedPreview
                html={previewHtml}
                className="bg-white rounded-xl shadow-lg mx-auto max-w-4xl p-10 min-h-[60vh] overflow-auto"
              />
            </div>
          )}
        </div>
      </div>

      {/* Toast */}
      {toast && (
        <div className="fixed bottom-6 right-6 bg-slate-900 border border-slate-700 rounded-xl px-4 py-3 shadow-2xl flex items-center gap-2 text-sm text-slate-200 z-50">
          <CheckCircle2 className="w-4 h-4 text-green-400" />
          {toast}
        </div>
      )}
    </div>
  );
}

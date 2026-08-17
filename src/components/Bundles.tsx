import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  listBundles,
  createBundle,
  deleteBundle,
  getBundleTemplates,
  listTemplates,
  exportTemplateFieldsCsv,
  batchFillFromCsv,
  BatchFillResult,
} from "../lib/ipc";
import type { Bundle, TemplateMeta } from "../lib/types";
import {
  FolderPlus,
  Trash2,
  Plus,
  Layers,
  FileSpreadsheet,
  Upload,
  Download,
  FileText,
  CheckCircle2,
  Loader2,
  FolderOpen,
} from "lucide-react";

interface Props {
  onUseTemplate?: (templateId: string) => void;
}

export default function Bundles({ onUseTemplate }: Props) {
  const [bundles, setBundles] = useState<Bundle[]>([]);
  const [templates, setTemplates] = useState<TemplateMeta[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);

  // Bundle detail (selected bundle) state
  const [selectedBundle, setSelectedBundle] = useState<Bundle | null>(null);
  const [memberTemplates, setMemberTemplates] = useState<TemplateMeta[]>([]);
  const [outputDir, setOutputDir] = useState("");
  const [formats, setFormats] = useState<{ docx: boolean; pdf: boolean }>({
    docx: true,
    pdf: false,
  });
  const [csvByTemplate, setCsvByTemplate] = useState<
    Record<string, { name: string; content: string }>
  >({});
  const [generating, setGenerating] = useState(false);
  const [results, setResults] = useState<Record<string, BatchFillResult | null>>({});

  const refresh = async () => {
    try {
      const [bs, ts] = await Promise.all([listBundles(), listTemplates()]);
      setBundles(bs);
      setTemplates(ts);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleCreate = async () => {
    setError(null);
    if (!name.trim()) {
      setError("Bundle name is required");
      return;
    }
    if (selected.size === 0) {
      setError("Select at least one template");
      return;
    }
    try {
      await createBundle({ name, description, templateIds: Array.from(selected) });
      setName("");
      setDescription("");
      setSelected(new Set());
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteBundle(id);
      if (selectedBundle?.id === id) {
        setSelectedBundle(null);
        setMemberTemplates([]);
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const selectBundle = async (b: Bundle) => {
    setSelectedBundle(b);
    setCsvByTemplate({});
    setResults({});
    setError(null);
    try {
      const ids = await getBundleTemplates(b.id);
      const members = templates.filter((t) => ids.includes(t.id));
      setMemberTemplates(members);
    } catch (e) {
      setError(String(e));
    }
  };

  const pickOutputDir = async () => {
    const sel = await open({ directory: true, multiple: false });
    if (sel && typeof sel === "string") setOutputDir(sel);
  };

  const handleExportTemplateCsv = async (t: TemplateMeta) => {
    try {
      const csv = await exportTemplateFieldsCsv(t.id);
      const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${t.name}_fields.csv`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(`Failed to export fields CSV for ${t.name}: ${e}`);
    }
  };

  const handleTemplateCsv = async (
    t: TemplateMeta,
    e: React.ChangeEvent<HTMLInputElement>
  ) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const content = await file.text();
    setCsvByTemplate((prev) => ({ ...prev, [t.id]: { name: file.name, content } }));
  };

  // Generate every bundle document: for each member template that has an uploaded
  // CSV, run the per-template batch fill into the shared output folder.
  const handleGenerateAll = async () => {
    if (!selectedBundle) return;
    if (!outputDir.trim()) {
      setError("Select an output folder before generating.");
      return;
    }
    const fmt: string[] = [];
    if (formats.docx) fmt.push("docx");
    if (formats.pdf) fmt.push("pdf");
    setGenerating(true);
    setError(null);
    const newResults: Record<string, BatchFillResult | null> = {};
    for (const t of memberTemplates) {
      const csv = csvByTemplate[t.id];
      if (!csv) continue;
      try {
        const res = await batchFillFromCsv({
          templateId: t.id,
          csv: csv.content,
          outputDir,
          formats: fmt,
        });
        newResults[t.id] = res;
      } catch (e) {
        newResults[t.id] = null;
        setError(`Failed generating ${t.name}: ${e}`);
      }
    }
    setResults(newResults);
    setGenerating(false);
  };

  const totalGenerated = Object.values(results).reduce(
    (sum, r) => sum + (r?.generated.length ?? 0),
    0
  );

  return (
    <div className="max-w-5xl mx-auto px-8 py-8 h-full overflow-y-auto">
      <div className="flex items-center gap-3 mb-6">
        <div className="w-10 h-10 bg-purple-600/20 border border-purple-500/30 rounded-xl flex items-center justify-center">
          <Layers className="w-5 h-5 text-purple-400" />
        </div>
        <div>
          <h2 className="text-2xl font-bold text-white">Template Bundles</h2>
          <p className="text-slate-400 text-sm">
            Group templates, then generate every document from a shared CSV or fill manually.
          </p>
        </div>
      </div>

      {error && (
        <div className="mb-4 p-3 rounded-lg bg-red-500/15 border border-red-500/30 text-red-300 text-sm">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Creator */}
        <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
          <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
            <FolderPlus className="w-4 h-4 text-purple-400" /> New Bundle
          </h3>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Bundle name"
            className="w-full mb-3 px-3 py-2 rounded-lg bg-slate-900/70 border border-slate-700 text-slate-100 text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Description (optional)"
            rows={2}
            className="w-full mb-3 px-3 py-2 rounded-lg bg-slate-900/70 border border-slate-700 text-slate-100 text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <div className="mb-3 text-xs font-semibold text-slate-400 uppercase tracking-wide">
            Select templates
          </div>
          <div className="max-h-56 overflow-y-auto space-y-1.5 mb-4">
            {templates.length === 0 && (
              <div className="text-slate-500 text-sm">No templates yet.</div>
            )}
            {templates.map((t) => (
              <label
                key={t.id}
                className="flex items-center gap-2 text-sm text-slate-200 bg-slate-900/50 rounded-lg px-3 py-2 cursor-pointer hover:bg-slate-900/80"
              >
                <input
                  type="checkbox"
                  checked={selected.has(t.id)}
                  onChange={() => toggle(t.id)}
                  className="accent-purple-500"
                />
                {t.name}
              </label>
            ))}
          </div>
          <button
            onClick={handleCreate}
            className="w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-purple-600 hover:bg-purple-500 text-white text-sm font-semibold transition"
          >
            <Plus className="w-4 h-4" /> Create Bundle
          </button>
        </div>

        {/* List */}
        <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
          <h3 className="text-lg font-semibold text-white mb-4">Existing Bundles</h3>
          <div className="space-y-2">
            {bundles.length === 0 && (
              <div className="text-slate-500 text-sm">No bundles created yet.</div>
            )}
            {bundles.map((b) => (
              <div
                key={b.id}
                className={`flex items-center justify-between bg-slate-900/50 rounded-lg px-4 py-3 cursor-pointer transition ${
                  selectedBundle?.id === b.id
                    ? "ring-2 ring-purple-500"
                    : "hover:bg-slate-900/80"
                }`}
                onClick={() => selectBundle(b)}
              >
                <div>
                  <div className="text-slate-100 font-medium">{b.name}</div>
                  <div className="text-slate-400 text-xs">
                    {b.description ? `${b.description} · ` : ""}Click to open
                  </div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDelete(b.id);
                  }}
                  className="p-2 text-slate-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition"
                  title="Delete bundle"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Bundle detail: per-template CSV upload + generate all */}
      {selectedBundle && (
        <div className="mt-6 bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-white flex items-center gap-2">
              <Layers className="w-4 h-4 text-purple-400" /> {selectedBundle.name} — Mail Merge
            </h3>
            <button
              onClick={() => {
                setSelectedBundle(null);
                setMemberTemplates([]);
                setCsvByTemplate({});
                setResults({});
              }}
              className="text-xs text-slate-400 hover:text-white transition"
            >
              Close
            </button>
          </div>

          <p className="text-slate-400 text-sm mb-4">
            For each template: export a blank CSV, fill it, then upload it. When all are ready,
            generate every document into one output folder. Or fill a template manually.
          </p>

          <div className="flex flex-wrap items-center gap-3 mb-4">
            <button
              onClick={pickOutputDir}
              className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 text-white px-3 py-2 rounded-lg text-sm transition"
            >
              <FolderOpen className="w-4 h-4" /> Output Folder
            </button>
            <span className="text-slate-400 text-xs truncate max-w-xs">
              {outputDir || "No folder selected"}
            </span>
            <label className="flex items-center gap-2 text-slate-300 text-sm ml-2">
              <input
                type="checkbox"
                checked={formats.docx}
                onChange={(e) => setFormats((f) => ({ ...f, docx: e.target.checked }))}
                className="accent-purple-500"
              />
              DOCX
            </label>
            <label className="flex items-center gap-2 text-slate-300 text-sm">
              <input
                type="checkbox"
                checked={formats.pdf}
                onChange={(e) => setFormats((f) => ({ ...f, pdf: e.target.checked }))}
                className="accent-purple-500"
              />
              PDF
            </label>
            <button
              onClick={handleGenerateAll}
              disabled={generating || Object.keys(csvByTemplate).length === 0 || !outputDir}
              className="flex items-center gap-2 bg-purple-600 hover:bg-purple-500 disabled:bg-slate-600 text-white px-4 py-2 rounded-lg text-sm font-semibold transition shadow-lg shadow-purple-600/20"
            >
              {generating ? <Loader2 className="w-4 h-4 animate-spin" /> : <FileSpreadsheet className="w-4 h-4" />}
              Generate All Docs from Bundle
            </button>
            {totalGenerated > 0 && (
              <span className="flex items-center gap-1 text-green-400 text-sm">
                <CheckCircle2 className="w-4 h-4" /> {totalGenerated} generated
              </span>
            )}
          </div>

          <div className="space-y-2">
            {memberTemplates.length === 0 && (
              <div className="text-slate-500 text-sm">This bundle has no templates.</div>
            )}
            {memberTemplates.map((t) => {
              const csv = csvByTemplate[t.id];
              const res = results[t.id];
              return (
                <div
                  key={t.id}
                  className="flex flex-col sm:flex-row sm:items-center gap-3 bg-slate-900/50 rounded-lg px-4 py-3"
                >
                  <div className="flex items-center gap-2 min-w-[180px]">
                    <FileText className="w-4 h-4 text-purple-400 shrink-0" />
                    <span className="text-slate-100 text-sm font-medium truncate">{t.name}</span>
                  </div>

                  <button
                    onClick={() => handleExportTemplateCsv(t)}
                    className="flex items-center gap-2 bg-slate-700 hover:bg-slate-600 text-white px-3 py-1.5 rounded-lg text-xs transition"
                  >
                    <Download className="w-3.5 h-3.5" /> Export Fields CSV
                  </button>

                  <label className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 text-white px-3 py-1.5 rounded-lg text-xs cursor-pointer transition">
                    <Upload className="w-3.5 h-3.5" /> Upload CSV
                    <input
                      type="file"
                      accept=".csv"
                      className="hidden"
                      onChange={(e) => handleTemplateCsv(t, e)}
                    />
                  </label>

                  {onUseTemplate && (
                    <button
                      onClick={() => onUseTemplate(t.id)}
                      className="text-xs text-purple-300 hover:text-white underline transition"
                    >
                      Fill manually
                    </button>
                  )}

                  <div className="text-xs text-slate-400 ml-auto text-right">
                    {csv ? (
                      <span className="text-green-400">{csv.name} ready</span>
                    ) : (
                      <span>No CSV</span>
                    )}
                    {res && (
                      <span className="ml-2 text-slate-500">
                        · {res.generated.length} doc(s)
                        {res.errors.length > 0 ? `, ${res.errors.length} err` : ""}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

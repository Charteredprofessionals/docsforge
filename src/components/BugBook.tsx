import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bug,
  Search,
  Download,
  FileText,
  Paperclip,
  Plus,
  X,
  AlertTriangle,
  Filter,
} from "lucide-react";
import {
  listBugs,
  createBugEntry,
  updateBugStatus,
  addBugAttachment,
  exportBugsCsv,
  exportBugsPdf,
  getBug,
} from "../lib/ipc";
import type { BugEntry, BugFilter, BugSeverity, BugStatus } from "../lib/types";

const SEVERITIES: BugSeverity[] = ["critical", "high", "medium", "low"];
const STATUSES: BugStatus[] = ["open", "in_progress", "resolved", "wont_fix"];

const severityClasses: Record<BugSeverity, string> = {
  critical: "bg-red-500/20 text-red-300 border-red-500/40",
  high: "bg-orange-500/20 text-orange-300 border-orange-500/40",
  medium: "bg-yellow-500/20 text-yellow-300 border-yellow-500/40",
  low: "bg-slate-500/20 text-slate-300 border-slate-500/40",
};

const statusClasses: Record<BugStatus, string> = {
  open: "bg-blue-500/20 text-blue-300 border-blue-500/40",
  in_progress: "bg-purple-500/20 text-purple-300 border-purple-500/40",
  resolved: "bg-green-500/20 text-green-300 border-green-500/40",
  wont_fix: "bg-slate-600/30 text-slate-400 border-slate-500/40",
};

function downloadBlob(content: BlobPart, filename: string, mime: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function base64ToBytes(b64: string): Uint8Array<ArrayBuffer> {
  const clean = b64.includes(",") ? b64.split(",")[1] : b64;
  const bin = atob(clean);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

export default function BugBook() {
  const [filter, setFilter] = useState<BugFilter>({
    sortBy: "createdAt",
    sortDir: "desc",
  });
  const [bugs, setBugs] = useState<BugEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<BugEntry | null>(null);
  const [showManual, setShowManual] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listBugs({
        dateFrom: filter.dateFrom,
        dateTo: filter.dateTo,
        severity: filter.severity || undefined,
        status: filter.status || undefined,
        keyword: filter.keyword,
        sortBy: filter.sortBy,
        sortDir: filter.sortDir,
      });
      setBugs(list);
    } catch (e) {
      setError(`Failed to load bug book: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    load();
  }, [load]);

  const refreshSelected = useCallback(async (id: string) => {
    try {
      const full = await getBug(id);
      setSelected(full);
      setBugs((prev) => prev.map((b) => (b.id === id ? full : b)));
    } catch {
      /* ignore */
    }
  }, []);

  const handleStatus = async (status: BugStatus) => {
    if (!selected) return;
    try {
      await updateBugStatus(selected.id, status, "admin");
      await refreshSelected(selected.id);
    } catch (e) {
      setError(`Status update failed: ${e}`);
    }
  };

  const handleAddAttachment = async (bugId: string, file: File) => {
    try {
      const dataUrl: string = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result as string);
        reader.onerror = reject;
        reader.readAsDataURL(file);
      });
      await addBugAttachment({
        bugId,
        filename: file.name,
        mimeType: file.type || "application/octet-stream",
        dataB64: dataUrl,
      });
      await refreshSelected(bugId);
    } catch (e) {
      setError(`Attachment failed: ${e}`);
    }
  };

  const handleExportCsv = async () => {
    try {
      const csv = await exportBugsCsv({
        dateFrom: filter.dateFrom,
        dateTo: filter.dateTo,
        severity: filter.severity || undefined,
        status: filter.status || undefined,
        keyword: filter.keyword,
        sortBy: filter.sortBy,
        sortDir: filter.sortDir,
      });
      downloadBlob(csv, `bug-book-${new Date().toISOString().slice(0, 10)}.csv`, "text/csv");
    } catch (e) {
      setError(`CSV export failed: ${e}`);
    }
  };

  const handleExportPdf = async () => {
    try {
      const { pdfBase64, filename } = await exportBugsPdf({
        dateFrom: filter.dateFrom,
        dateTo: filter.dateTo,
        severity: filter.severity || undefined,
        status: filter.status || undefined,
        keyword: filter.keyword,
        sortBy: filter.sortBy,
        sortDir: filter.sortDir,
      });
      downloadBlob(base64ToBytes(pdfBase64), filename, "application/pdf");
    } catch (e) {
      setError(`PDF export failed: ${e}`);
    }
  };

  const counts = useMemo(() => {
    const c: Record<string, number> = { critical: 0, high: 0, medium: 0, low: 0 };
    bugs.forEach((b) => {
      c[b.severity] = (c[b.severity] ?? 0) + 1;
    });
    return c;
  }, [bugs]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Bug className="w-5 h-5 text-red-400" />
          <h3 className="text-lg font-semibold">Bug Book</h3>
          <span className="text-xs text-slate-500">
            {bugs.length} entries · crit {counts.critical} / high {counts.high}
          </span>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setShowManual(true)}
            className="flex items-center gap-1.5 bg-blue-600 hover:bg-blue-500 text-white px-3 py-1.5 rounded-lg text-sm font-medium transition"
          >
            <Plus className="w-4 h-4" /> New Bug
          </button>
          <button
            onClick={handleExportCsv}
            className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 text-white px-3 py-1.5 rounded-lg text-sm transition"
          >
            <Download className="w-4 h-4" /> CSV
          </button>
          <button
            onClick={handleExportPdf}
            className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 text-white px-3 py-1.5 rounded-lg text-sm transition"
          >
            <Download className="w-4 h-4" /> PDF
          </button>
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex flex-wrap items-end gap-3 bg-slate-950/60 p-3 rounded-lg border border-slate-800">
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">From</label>
          <input
            type="date"
            value={filter.dateFrom ?? ""}
            onChange={(e) => setFilter((f) => ({ ...f, dateFrom: e.target.value || undefined }))}
            className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">To</label>
          <input
            type="date"
            value={filter.dateTo ?? ""}
            onChange={(e) => setFilter((f) => ({ ...f, dateTo: e.target.value || undefined }))}
            className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Severity</label>
          <select
            value={filter.severity ?? ""}
            onChange={(e) => setFilter((f) => ({ ...f, severity: (e.target.value || undefined) as BugSeverity | "" }))}
            className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          >
            <option value="">All</option>
            {SEVERITIES.map((s) => (
              <option key={s} value={s}>{s}</option>
            ))}
          </select>
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Status</label>
          <select
            value={filter.status ?? ""}
            onChange={(e) => setFilter((f) => ({ ...f, status: (e.target.value || undefined) as BugStatus | "" }))}
            className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          >
            <option value="">All</option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>{s.replace("_", " ")}</option>
            ))}
          </select>
        </div>
        <div className="flex flex-col gap-1 flex-1 min-w-[160px]">
          <label className="text-xs text-slate-400">Keyword</label>
          <div className="flex gap-1">
            <input
              type="text"
              placeholder="message, type, context…"
              value={filter.keyword ?? ""}
              onChange={(e) => setFilter((f) => ({ ...f, keyword: e.target.value }))}
              onKeyDown={(e) => e.key === "Enter" && load()}
              className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white flex-1"
            />
            <button
              onClick={load}
              className="flex items-center gap-1 bg-blue-600 hover:bg-blue-500 text-white px-2.5 rounded text-sm"
            >
              <Search className="w-4 h-4" />
            </button>
          </div>
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Sort</label>
          <div className="flex gap-1">
            <select
              value={filter.sortBy ?? "createdAt"}
              onChange={(e) => setFilter((f) => ({ ...f, sortBy: e.target.value as BugFilter["sortBy"] }))}
              className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-sm text-white"
            >
              <option value="createdAt">Date</option>
              <option value="severity">Severity</option>
              <option value="status">Status</option>
              <option value="errorType">Type</option>
            </select>
            <button
              onClick={() => setFilter((f) => ({ ...f, sortDir: f.sortDir === "asc" ? "desc" : "asc" }))}
              className="bg-slate-800 hover:bg-slate-700 text-white px-2 rounded text-sm"
              title="Toggle direction"
            >
              {filter.sortDir === "asc" ? "↑" : "↓"}
            </button>
          </div>
        </div>
      </div>

      {error && <div className="text-red-400 text-sm">{error}</div>}
      {loading && <div className="text-slate-400 text-sm">Loading…</div>}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* List */}
        <div className="lg:col-span-2 bg-slate-950/60 border border-slate-800 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-slate-400 border-b border-slate-800">
                <th className="px-3 py-2 font-medium">Created</th>
                <th className="px-3 py-2 font-medium">Sev</th>
                <th className="px-3 py-2 font-medium">Status</th>
                <th className="px-3 py-2 font-medium">Type</th>
                <th className="px-3 py-2 font-medium">Message</th>
              </tr>
            </thead>
            <tbody>
              {bugs.map((b) => (
                <tr
                  key={b.id}
                  onClick={() => setSelected(b)}
                  className={`border-b border-slate-800/60 cursor-pointer hover:bg-slate-800/40 ${
                    selected?.id === b.id ? "bg-slate-800/60" : ""
                  }`}
                >
                  <td className="px-3 py-2 text-slate-400 whitespace-nowrap">{b.createdAt.replace("T", " ").slice(0, 16)}</td>
                  <td className="px-3 py-2">
                    <span className={`px-2 py-0.5 rounded border text-xs font-medium ${severityClasses[b.severity]}`}>
                      {b.severity}
                    </span>
                  </td>
                  <td className="px-3 py-2">
                    <span className={`px-2 py-0.5 rounded border text-xs font-medium ${statusClasses[b.status]}`}>
                      {b.status.replace("_", " ")}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-slate-300">{b.errorType}</td>
                  <td className="px-3 py-2 text-slate-300 max-w-[220px] truncate" title={b.message}>{b.message}</td>
                </tr>
              ))}
              {bugs.length === 0 && !loading && (
                <tr>
                  <td colSpan={5} className="px-3 py-8 text-center text-slate-500">
                    No bug entries match the current filters.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        {/* Detail */}
        <div className="bg-slate-950/60 border border-slate-800 rounded-xl p-4">
          {selected ? (
            <BugDetail
              bug={selected}
              onStatus={handleStatus}
              onAddAttachment={(file) => handleAddAttachment(selected.id, file)}
            />
          ) : (
            <div className="text-slate-500 text-sm flex flex-col items-center justify-center h-full gap-2 py-10">
              <Filter className="w-8 h-8 text-slate-600" />
              Select an entry to view details, update status, or attach files.
            </div>
          )}
        </div>
      </div>

      {showManual && (
        <ManualBugModal
          onClose={() => setShowManual(false)}
          onCreated={async () => {
            setShowManual(false);
            await load();
          }}
        />
      )}
    </div>
  );
}

function BugDetail({
  bug,
  onStatus,
  onAddAttachment,
}: {
  bug: BugEntry;
  onStatus: (s: BugStatus) => void;
  onAddAttachment: (file: File) => void;
}) {
  const [attachment, setAttachment] = useState<File | null>(null);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className={`px-2 py-0.5 rounded border text-xs font-medium ${severityClasses[bug.severity]}`}>
          {bug.severity}
        </span>
        <span className={`px-2 py-0.5 rounded border text-xs font-medium ${statusClasses[bug.status]}`}>
          {bug.status.replace("_", " ")}
        </span>
      </div>

      <div>
        <div className="text-xs text-slate-500">Type</div>
        <div className="text-sm text-slate-200">{bug.errorType} · <span className="text-slate-400">{bug.source}</span></div>
      </div>
      <div>
        <div className="text-xs text-slate-500">Context</div>
        <div className="text-sm text-slate-200 break-words">{bug.context || "—"}</div>
      </div>
      <div>
        <div className="text-xs text-slate-500">Message</div>
        <div className="text-sm text-slate-200 break-words">{bug.message}</div>
      </div>
      {bug.stackTrace && (
        <div>
          <div className="text-xs text-slate-500 mb-1">Stack Trace</div>
          <pre className="text-xs text-slate-300 bg-slate-900 border border-slate-800 rounded p-2 max-h-40 overflow-auto whitespace-pre-wrap">
            {bug.stackTrace}
          </pre>
        </div>
      )}

      <div>
        <div className="text-xs text-slate-500 mb-1">Attachments ({bug.attachments.length})</div>
        <ul className="space-y-1">
          {bug.attachments.map((a) => (
            <li key={a.id} className="flex items-center gap-2 text-sm text-slate-300">
              <Paperclip className="w-3.5 h-3.5 text-slate-500" />
              <FileText className="w-3.5 h-3.5 text-slate-500" />
              {a.filename}
            </li>
          ))}
          {bug.attachments.length === 0 && <li className="text-xs text-slate-600">None</li>}
        </ul>
        <label className="mt-2 inline-flex items-center gap-1.5 text-xs text-blue-300 cursor-pointer hover:text-blue-200">
          <Paperclip className="w-3.5 h-3.5" /> Attach log / screenshot
          <input
            type="file"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) onAddAttachment(f);
            }}
          />
        </label>
      </div>

      <div>
        <div className="text-xs text-slate-500 mb-1">Update Status</div>
        <div className="flex flex-wrap gap-1.5">
          {STATUSES.map((s) => (
            <button
              key={s}
              onClick={() => onStatus(s)}
              className={`px-2.5 py-1 rounded text-xs font-medium border transition ${
                bug.status === s ? statusClasses[s] : "border-slate-700 text-slate-400 hover:text-white hover:border-slate-500"
              }`}
            >
              {s.replace("_", " ")}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function ManualBugModal({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: () => void;
}) {
  const [form, setForm] = useState({
    errorType: "manual_report",
    severity: "medium" as BugSeverity,
    status: "open" as BugStatus,
    context: "",
    message: "",
    stackTrace: "",
    category: "",
    keywords: "",
  });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const submit = async () => {
    if (!form.message.trim()) {
      setErr("A description is required.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      await createBugEntry({
        errorType: form.errorType,
        message: form.message,
        severity: form.severity,
        status: form.status,
        context: form.context || undefined,
        stackTrace: form.stackTrace || undefined,
        category: form.category || undefined,
        keywords: form.keywords || undefined,
      });
      onCreated();
    } catch (e) {
      setErr(`Failed to create bug: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-lg p-5 space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <AlertTriangle className="w-5 h-5 text-yellow-400" />
            <h4 className="text-lg font-semibold">New Bug Entry</h4>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-slate-400">Error Type</label>
            <input
              value={form.errorType}
              onChange={(e) => setForm((f) => ({ ...f, errorType: e.target.value }))}
              className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
            />
          </div>
          <div className="flex gap-2">
            <div className="flex flex-col gap-1 flex-1">
              <label className="text-xs text-slate-400">Severity</label>
              <select
                value={form.severity}
                onChange={(e) => setForm((f) => ({ ...f, severity: e.target.value as BugSeverity }))}
                className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
              >
                {SEVERITIES.map((s) => (
                  <option key={s} value={s}>{s}</option>
                ))}
              </select>
            </div>
            <div className="flex flex-col gap-1 flex-1">
              <label className="text-xs text-slate-400">Status</label>
              <select
                value={form.status}
                onChange={(e) => setForm((f) => ({ ...f, status: e.target.value as BugStatus }))}
                className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
              >
                {STATUSES.map((s) => (
                  <option key={s} value={s}>{s.replace("_", " ")}</option>
                ))}
              </select>
            </div>
          </div>
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Context (affected user / endpoint)</label>
          <input
            value={form.context}
            onChange={(e) => setForm((f) => ({ ...f, context: e.target.value }))}
            placeholder="e.g. user:admin@endpoint:/admin"
            className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Description *</label>
          <textarea
            value={form.message}
            onChange={(e) => setForm((f) => ({ ...f, message: e.target.value }))}
            rows={3}
            className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
          />
        </div>
        <div className="flex flex-col gap-1">
          <label className="text-xs text-slate-400">Stack Trace</label>
          <textarea
            value={form.stackTrace}
            onChange={(e) => setForm((f) => ({ ...f, stackTrace: e.target.value }))}
            rows={3}
            className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white font-mono"
          />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-slate-400">Category</label>
            <input
              value={form.category}
              onChange={(e) => setForm((f) => ({ ...f, category: e.target.value }))}
              className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
            />
          </div>
          <div className="flex flex-col gap-1">
            <label className="text-xs text-slate-400">Keywords (comma-sep)</label>
            <input
              value={form.keywords}
              onChange={(e) => setForm((f) => ({ ...f, keywords: e.target.value }))}
              className="bg-slate-950 border border-slate-700 rounded px-2 py-1 text-sm text-white"
            />
          </div>
        </div>

        {err && <div className="text-red-400 text-sm">{err}</div>}

        <div className="flex justify-end gap-2 pt-1">
          <button onClick={onClose} className="px-3 py-1.5 rounded-lg text-sm text-slate-300 hover:text-white">
            Cancel
          </button>
          <button
            onClick={submit}
            disabled={busy}
            className="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white px-4 py-1.5 rounded-lg text-sm font-medium"
          >
            {busy ? "Saving…" : "Create Bug"}
          </button>
        </div>
      </div>
    </div>
  );
}

import React, { useEffect, useState } from "react";
import {
  listBundles,
  createBundle,
  deleteBundle,
  getBundleTemplates,
  listTemplates,
} from "../lib/ipc";
import type { Bundle, TemplateMeta } from "../lib/types";
import { FolderPlus, Trash2, Plus, Layers } from "lucide-react";

export default function Bundles() {
  const [bundles, setBundles] = useState<Bundle[]>([]);
  const [templates, setTemplates] = useState<TemplateMeta[]>([]);
  const [bundleMembers, setBundleMembers] = useState<Record<string, number>>({});
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [bs, ts] = await Promise.all([listBundles(), listTemplates()]);
      setBundles(bs);
      setTemplates(ts);
      const counts: Record<string, number> = {};
      for (const b of bs) {
        const ids = await getBundleTemplates(b.id);
        counts[b.id] = ids.length;
      }
      setBundleMembers(counts);
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
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="max-w-5xl mx-auto px-8 py-8 h-full overflow-y-auto">
      <div className="flex items-center gap-3 mb-6">
        <div className="w-10 h-10 bg-purple-600/20 border border-purple-500/30 rounded-xl flex items-center justify-center">
          <Layers className="w-5 h-5 text-purple-400" />
        </div>
        <div>
          <h2 className="text-2xl font-bold text-white">Template Bundles</h2>
          <p className="text-slate-400 text-sm">Group templates to process them together.</p>
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
                className="flex items-center justify-between bg-slate-900/50 rounded-lg px-4 py-3"
              >
                <div>
                  <div className="text-slate-100 font-medium">{b.name}</div>
                  <div className="text-slate-400 text-xs">
                    {bundleMembers[b.id] ?? 0} template(s)
                    {b.description ? ` · ${b.description}` : ""}
                  </div>
                </div>
                <button
                  onClick={() => handleDelete(b.id)}
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
    </div>
  );
}

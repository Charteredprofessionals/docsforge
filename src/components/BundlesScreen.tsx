import React, { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  createBundleV2,
  listBundlesV2,
  getBundleV2,
  publishVersion,
  exportBundleDfpkg,
  importBundleDfpkg,
  listVersions,
  findUnmappedPlaceholders,
} from "../lib/ipc";
import type {
  BundleSummary,
  BundleDetail,
  BundleVersion,
  UnmappedPlaceholder,
} from "../lib/types";
import {
  Layers,
  Plus,
  Upload,
  Download,
  Package,
  AlertTriangle,
  CheckCircle,
  FileText,
  GitBranch,
  Loader2,
  Archive,
  Eye,
  Rocket,
} from "lucide-react";

interface Props {
  onViewMatter?: (bundleId: string) => void;
}

export default function BundlesScreen({ onViewMatter }: Props) {
  const [bundles, setBundles] = useState<BundleSummary[]>([]);
  const [selectedBundle, setSelectedBundle] = useState<BundleDetail | null>(null);
  const [versions, setVersions] = useState<BundleVersion[]>([]);
  const [unmappedPlaceholders, setUnmappedPlaceholders] = useState<UnmappedPlaceholder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  
  // Create bundle form
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [creating, setCreating] = useState(false);

  // Publish form
  const [showPublishForm, setShowPublishForm] = useState(false);
  const [publishNote, setPublishNote] = useState("");
  const [publishing, setPublishing] = useState(false);

  // Export/Import
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    loadBundles();
  }, []);

  const loadBundles = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await listBundlesV2();
      setBundles(data);
    } catch (e) {
      setError(`Failed to load bundles: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const loadBundleDetail = async (bundleId: string) => {
    try {
      setError(null);
      const [detail, versionList, unmapped] = await Promise.all([
        getBundleV2(bundleId),
        listVersions(bundleId),
        findUnmappedPlaceholders(bundleId),
      ]);
      setSelectedBundle(detail);
      setVersions(versionList);
      setUnmappedPlaceholders(unmapped);
    } catch (e) {
      setError(`Failed to load bundle details: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleCreate = async () => {
    if (!name.trim()) {
      setError("Bundle name is required");
      return;
    }

    try {
      setCreating(true);
      setError(null);
      await createBundleV2(name, description);
      setName("");
      setDescription("");
      setShowCreateForm(false);
      await loadBundles();
    } catch (e) {
      setError(`Failed to create bundle: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setCreating(false);
    }
  };

  const handlePublish = async () => {
    if (!selectedBundle) return;
    if (!publishNote.trim()) {
      setError("Please provide a version note");
      return;
    }

    if (!confirm(`Publish version for "${selectedBundle.name}"? This will create an immutable version.`)) {
      return;
    }

    try {
      setPublishing(true);
      setError(null);
      await publishVersion(selectedBundle.id, publishNote);
      setPublishNote("");
      setShowPublishForm(false);
      await Promise.all([
        loadBundles(),
        loadBundleDetail(selectedBundle.id),
      ]);
    } catch (e) {
      setError(`Failed to publish version: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setPublishing(false);
    }
  };

  const handleExport = async () => {
    if (!selectedBundle) return;

    const latestVersion = versions.find((v) => v.status === "published");
    if (!latestVersion) {
      setError("No published version available for export");
      return;
    }

    try {
      setExporting(true);
      setError(null);

      const savePath = await save({
        defaultPath: `${selectedBundle.name.replace(/\s+/g, "_")}_v${latestVersion.versionNumber}.dfpkg`,
        filters: [{
          name: "DocForge Package",
          extensions: ["dfpkg"],
        }],
      });

      if (savePath) {
        const bytes = await exportBundleDfpkg(selectedBundle.id, latestVersion.id);
        
        // Write to file
        // Note: In real implementation, use Tauri's fs plugin to write the file
        alert(`Export successful! Would save ${bytes.length} bytes to ${savePath}`);
      }
    } catch (e) {
      setError(`Export failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [{
          name: "DocForge Package",
          extensions: ["dfpkg"],
        }],
      });

      if (filePath && typeof filePath === "string") {
        setImporting(true);
        setError(null);

        // Note: In real implementation, read the file using Tauri's fs plugin
        // For now, show placeholder
        alert(`Would import bundle from: ${filePath}`);

        // const bytes = await readBinaryFile(filePath);
        // await importBundleDfpkg(new Uint8Array(bytes));
        // await loadBundles();
      }
    } catch (e) {
      setError(`Import failed: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setImporting(false);
    }
  };

  const getStatusBadge = (status: BundleSummary["status"] | BundleVersion["status"]) => {
    switch (status) {
      case "published":
        return <span className="px-2 py-0.5 rounded text-xs font-medium bg-green-500/15 border border-green-500/30 text-green-300">Published</span>;
      case "draft":
        return <span className="px-2 py-0.5 rounded text-xs font-medium bg-yellow-500/15 border border-yellow-500/30 text-yellow-300">Draft</span>;
      case "review":
        return <span className="px-2 py-0.5 rounded text-xs font-medium bg-blue-500/15 border border-blue-500/30 text-blue-300">Review</span>;
      case "archived":
        return <span className="px-2 py-0.5 rounded text-xs font-medium bg-slate-500/15 border border-slate-500/30 text-slate-300">Archived</span>;
      default:
        return null;
    }
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return dateStr;
    }
  };

  if (loading) {
    return (
      <div className="max-w-7xl mx-auto px-8 py-8 flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
      </div>
    );
  }

  return (
    <div className="max-w-7xl mx-auto px-8 py-8 h-full overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-purple-600/20 border border-purple-500/30 rounded-xl flex items-center justify-center">
            <Layers className="w-5 h-5 text-purple-400" />
          </div>
          <div>
            <h2 className="text-2xl font-bold text-white">Bundle Management</h2>
            <p className="text-slate-400 text-sm">
              Create, version, and distribute document bundles
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleImport}
            disabled={importing}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 text-white text-sm transition"
          >
            {importing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Upload className="w-4 h-4" />}
            Import .dfpkg
          </button>
          <button
            onClick={() => setShowCreateForm(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-purple-600 hover:bg-purple-500 text-white text-sm font-semibold transition shadow-lg shadow-purple-600/20"
          >
            <Plus className="w-4 h-4" />
            New Bundle
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-6 p-4 rounded-lg bg-red-500/15 border border-red-500/30 text-red-300 text-sm">
          <AlertTriangle className="w-4 h-4 inline mr-2" />
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Bundle List */}
        <div className="lg:col-span-1">
          <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
            <h3 className="text-lg font-semibold text-white mb-4">Bundles</h3>
            <div className="space-y-2 max-h-[600px] overflow-y-auto">
              {bundles.length === 0 && (
                <div className="text-slate-500 text-sm py-8 text-center">
                  No bundles yet
                </div>
              )}
              {bundles.map((bundle) => (
                <button
                  key={bundle.id}
                  onClick={() => loadBundleDetail(bundle.id)}
                  className={`w-full text-left p-4 rounded-lg border transition ${
                    selectedBundle?.id === bundle.id
                      ? "bg-purple-500/20 border-purple-500"
                      : "bg-slate-900/50 border-slate-700 hover:bg-slate-800/50"
                  }`}
                >
                  <div className="flex items-start justify-between mb-1">
                    <div className="font-medium text-white">{bundle.name}</div>
                    {getStatusBadge(bundle.status)}
                  </div>
                  {bundle.description && (
                    <div className="text-xs text-slate-400 mb-2">{bundle.description}</div>
                  )}
                  <div className="flex items-center gap-3 text-xs text-slate-500">
                    <span>{bundle.versionCount} version{bundle.versionCount !== 1 ? "s" : ""}</span>
                    <span>·</span>
                    <span>{formatDate(bundle.updatedAt)}</span>
                  </div>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Bundle Detail */}
        <div className="lg:col-span-2">
          {!selectedBundle ? (
            <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-12 text-center">
              <Package className="w-16 h-16 text-slate-600 mx-auto mb-4" />
              <p className="text-slate-400 text-sm">Select a bundle to view details</p>
            </div>
          ) : (
            <div className="space-y-6">
              {/* Bundle Info */}
              <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
                <div className="flex items-start justify-between mb-4">
                  <div>
                    <div className="flex items-center gap-2 mb-2">
                      <h3 className="text-xl font-bold text-white">{selectedBundle.name}</h3>
                      {getStatusBadge(selectedBundle.status)}
                    </div>
                    {selectedBundle.description && (
                      <p className="text-slate-400 text-sm">{selectedBundle.description}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={handleExport}
                      disabled={exporting || !versions.some((v) => v.status === "published")}
                      className="flex items-center gap-2 px-3 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 text-white text-sm transition"
                      title="Export .dfpkg"
                    >
                      {exporting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Download className="w-4 h-4" />}
                    </button>
                    <button
                      onClick={() => setShowPublishForm(true)}
                      className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-sm font-semibold transition shadow-lg shadow-blue-600/20"
                    >
                      <Rocket className="w-4 h-4" />
                      Publish Version
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-3 gap-4">
                  <div className="bg-slate-900/50 rounded-lg p-3 border border-slate-700">
                    <div className="text-slate-400 text-xs mb-1">Documents</div>
                    <div className="text-xl font-bold text-white">{selectedBundle.documents.length}</div>
                  </div>
                  <div className="bg-slate-900/50 rounded-lg p-3 border border-slate-700">
                    <div className="text-slate-400 text-xs mb-1">Versions</div>
                    <div className="text-xl font-bold text-white">{selectedBundle.versionCount}</div>
                  </div>
                  <div className="bg-slate-900/50 rounded-lg p-3 border border-slate-700">
                    <div className="text-slate-400 text-xs mb-1">Current</div>
                    <div className="text-xl font-bold text-white">
                      v{selectedBundle.currentVersion || "0"}
                    </div>
                  </div>
                </div>
              </div>

              {/* Health Check */}
              {unmappedPlaceholders.length > 0 && (
                <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-2xl p-5">
                  <div className="flex items-center gap-2 text-yellow-300 font-semibold mb-3">
                    <AlertTriangle className="w-5 h-5" />
                    Health Check: Unmapped Placeholders
                  </div>
                  <div className="space-y-2">
                    {unmappedPlaceholders.map((item, idx) => (
                      <div key={idx} className="bg-slate-900/50 rounded-lg p-3 text-sm">
                        <div className="text-white font-medium">{item.documentName}</div>
                        <div className="text-slate-400">
                          Placeholder: <span className="font-mono text-yellow-300">{item.placeholder}</span>
                          {item.count > 1 && <span className="ml-2">({item.count} occurrences)</span>}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {unmappedPlaceholders.length === 0 && (
                <div className="bg-green-500/10 border border-green-500/30 rounded-2xl p-4">
                  <div className="flex items-center gap-2 text-green-300">
                    <CheckCircle className="w-5 h-5" />
                    <span className="font-semibold">All placeholders mapped</span>
                  </div>
                </div>
              )}

              {/* Documents */}
              <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
                <h4 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
                  <FileText className="w-5 h-5 text-purple-400" />
                  Documents
                </h4>
                {selectedBundle.documents.length === 0 ? (
                  <div className="text-slate-500 text-sm text-center py-4">
                    No documents in this bundle
                  </div>
                ) : (
                  <div className="space-y-2">
                    {selectedBundle.documents.map((doc) => (
                      <div
                        key={doc.id}
                        className="bg-slate-900/50 rounded-lg p-3 border border-slate-700 flex items-center justify-between"
                      >
                        <div className="flex items-center gap-2">
                          <FileText className="w-4 h-4 text-purple-400" />
                          <span className="text-sm text-white">{doc.name}</span>
                        </div>
                        <button
                          className="text-xs text-purple-300 hover:text-white underline"
                          onClick={() => alert(`View document: ${doc.id}`)}
                        >
                          <Eye className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* Version History */}
              <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
                <h4 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
                  <GitBranch className="w-5 h-5 text-blue-400" />
                  Version History
                </h4>
                {versions.length === 0 ? (
                  <div className="text-slate-500 text-sm text-center py-4">
                    No versions published yet
                  </div>
                ) : (
                  <div className="space-y-2">
                    {versions.map((version) => (
                      <div
                        key={version.id}
                        className="bg-slate-900/50 rounded-lg p-3 border border-slate-700"
                      >
                        <div className="flex items-start justify-between">
                          <div>
                            <div className="flex items-center gap-2 mb-1">
                              <span className="text-sm font-semibold text-white">
                                Version {version.versionNumber}
                              </span>
                              {getStatusBadge(version.status)}
                            </div>
                            <div className="text-xs text-slate-400">{version.note}</div>
                            <div className="text-xs text-slate-500 mt-1">
                              {formatDate(version.createdAt)}
                              {version.publishedAt && ` · Published ${formatDate(version.publishedAt)}`}
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Create Bundle Modal */}
      {showCreateForm && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-slate-800 border border-slate-700 rounded-2xl p-6 max-w-md w-full">
            <h3 className="text-xl font-bold text-white mb-4">Create New Bundle</h3>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1.5">
                  Bundle Name <span className="text-red-400">*</span>
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g., Employment Package"
                  className="w-full px-3 py-2 rounded-lg bg-slate-900/70 border border-slate-700 text-slate-100 text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1.5">
                  Description
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Optional description..."
                  rows={3}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900/70 border border-slate-700 text-slate-100 text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
                />
              </div>
              <div className="flex items-center gap-3 pt-2">
                <button
                  onClick={handleCreate}
                  disabled={creating}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-slate-600 text-white font-semibold transition"
                >
                  {creating ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                  Create Bundle
                </button>
                <button
                  onClick={() => {
                    setShowCreateForm(false);
                    setName("");
                    setDescription("");
                  }}
                  className="px-4 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 text-white transition"
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Publish Version Modal */}
      {showPublishForm && selectedBundle && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
          <div className="bg-slate-800 border border-slate-700 rounded-2xl p-6 max-w-md w-full">
            <h3 className="text-xl font-bold text-white mb-2">Publish Version</h3>
            <p className="text-sm text-slate-400 mb-4">
              This will create an immutable published version of "{selectedBundle.name}".
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-slate-300 mb-1.5">
                  Version Note <span className="text-red-400">*</span>
                </label>
                <textarea
                  value={publishNote}
                  onChange={(e) => setPublishNote(e.target.value)}
                  placeholder="e.g., Initial release, Added new documents..."
                  rows={3}
                  className="w-full px-3 py-2 rounded-lg bg-slate-900/70 border border-slate-700 text-slate-100 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
              </div>
              <div className="flex items-center gap-3 pt-2">
                <button
                  onClick={handlePublish}
                  disabled={publishing}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:bg-slate-600 text-white font-semibold transition"
                >
                  {publishing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Rocket className="w-4 h-4" />}
                  Publish Version
                </button>
                <button
                  onClick={() => {
                    setShowPublishForm(false);
                    setPublishNote("");
                  }}
                  className="px-4 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 text-white transition"
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

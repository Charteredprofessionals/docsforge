import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { save } from "@tauri-apps/plugin-dialog";
import {
  listGenerationRuns,
  evaluatePreview,
  executeRun,
  getRun,
} from "../lib/ipc";
import type {
  GenerationRun,
  GenerationPreview,
  ExecuteResult,
} from "../lib/types";
import {
  History,
  Play,
  Download,
  FileText,
  CheckCircle2,
  XCircle,
  AlertCircle,
  Loader2,
  RefreshCw,
  Clock,
  Package,
} from "lucide-react";

interface Props {
  matterId: string;
  onBack: () => void;
}

export default function GenerationHistory({ matterId, onBack }: Props) {
  const [runs, setRuns] = useState<GenerationRun[]>([]);
  const [preview, setPreview] = useState<GenerationPreview | null>(null);
  const [generating, setGenerating] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedRun, setSelectedRun] = useState<GenerationRun | null>(null);

  useEffect(() => {
    loadData();
  }, [matterId]);

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);
      const [runsData, previewData] = await Promise.all([
        listGenerationRuns(matterId),
        evaluatePreview(matterId),
      ]);
      setRuns(runsData);
      setPreview(previewData);
    } catch (e) {
      setError(`Failed to load data: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleGenerate = async (documentIds?: string[]) => {
    try {
      setGenerating(true);
      setError(null);
      const result: ExecuteResult = await executeRun(matterId, documentIds);
      
      if (result.status === "success") {
        await loadData();
      } else if (result.status === "partial") {
        setError(`Generation partially complete. Errors: ${result.errors.join(", ")}`);
        await loadData();
      } else {
        setError(`Generation failed: ${result.errors.join(", ")}`);
      }
    } catch (e) {
      setError(`Generation error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setGenerating(false);
    }
  };

  const handleDownloadDocument = async (doc: { outputPath: string; documentName: string; format: string }) => {
    try {
      const savePath = await save({
        defaultPath: `${doc.documentName}.${doc.format}`,
        filters: [{
          name: doc.format.toUpperCase(),
          extensions: [doc.format],
        }],
      });

      if (savePath) {
        // In a real implementation, we'd copy the file from doc.outputPath to savePath
        // For now, we'll show a notification
        alert(`Would download: ${doc.outputPath} to ${savePath}`);
      }
    } catch (e) {
      setError(`Download failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const handleRerun = async (run: GenerationRun) => {
    if (confirm(`Create a new generation run based on ${run.id}?`)) {
      await handleGenerate();
    }
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const getStatusIcon = (status: GenerationRun["status"]) => {
    switch (status) {
      case "success":
        return <CheckCircle2 className="w-4 h-4 text-green-400" />;
      case "failed":
        return <XCircle className="w-4 h-4 text-red-400" />;
      case "running":
        return <Loader2 className="w-4 h-4 text-blue-400 animate-spin" />;
      case "pending":
        return <Clock className="w-4 h-4 text-yellow-400" />;
      default:
        return <AlertCircle className="w-4 h-4 text-slate-400" />;
    }
  };

  const getStatusColor = (status: GenerationRun["status"]) => {
    switch (status) {
      case "success":
        return "bg-green-500/15 border-green-500/30 text-green-300";
      case "failed":
        return "bg-red-500/15 border-red-500/30 text-red-300";
      case "running":
        return "bg-blue-500/15 border-blue-500/30 text-blue-300";
      case "pending":
        return "bg-yellow-500/15 border-yellow-500/30 text-yellow-300";
      default:
        return "bg-slate-500/15 border-slate-500/30 text-slate-300";
    }
  };

  if (loading) {
    return (
      <div className="max-w-6xl mx-auto px-8 py-8 flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
      </div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto px-8 py-8 h-full overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-purple-600/20 border border-purple-500/30 rounded-xl flex items-center justify-center">
            <History className="w-5 h-5 text-purple-400" />
          </div>
          <div>
            <h2 className="text-2xl font-bold text-white">Document Generation</h2>
            <p className="text-slate-400 text-sm">Preview and execute generation runs</p>
          </div>
        </div>
        <button
          onClick={onBack}
          className="px-4 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 text-white text-sm transition"
        >
          Back to Matter
        </button>
      </div>

      {error && (
        <div className="mb-6 p-4 rounded-lg bg-red-500/15 border border-red-500/30 text-red-300 text-sm">
          <AlertCircle className="w-4 h-4 inline mr-2" />
          {error}
        </div>
      )}

      {/* Preview Section */}
      {preview && (
        <div className="mb-6 bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-white flex items-center gap-2">
              <Package className="w-5 h-5 text-blue-400" />
              Generation Preview
            </h3>
            <button
              onClick={() => handleGenerate()}
              disabled={generating || preview.includedCount === 0}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:bg-slate-600 text-white font-semibold transition shadow-lg shadow-blue-600/20"
            >
              {generating ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Generating...
                </>
              ) : (
                <>
                  <Play className="w-4 h-4" />
                  Generate Documents
                </>
              )}
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
            <div className="bg-slate-900/50 rounded-lg p-4 border border-slate-700">
              <div className="text-slate-400 text-xs uppercase tracking-wide mb-1">Total Documents</div>
              <div className="text-2xl font-bold text-white">{preview.totalDocuments}</div>
            </div>
            <div className="bg-green-500/10 border border-green-500/30 rounded-lg p-4">
              <div className="text-green-400 text-xs uppercase tracking-wide mb-1">Will Generate</div>
              <div className="text-2xl font-bold text-green-300">{preview.includedCount}</div>
            </div>
            <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-4">
              <div className="text-yellow-400 text-xs uppercase tracking-wide mb-1">Skipped</div>
              <div className="text-2xl font-bold text-yellow-300">{preview.skippedCount}</div>
            </div>
          </div>

          {preview.skipped.length > 0 && (
            <div>
              <h4 className="text-sm font-semibold text-slate-300 mb-2">Skipped Documents</h4>
              <div className="space-y-2">
                {preview.skipped.map((doc) => (
                  <div
                    key={doc.documentId}
                    className="bg-slate-900/50 rounded-lg p-3 border border-yellow-500/20"
                  >
                    <div className="flex items-start gap-2">
                      <XCircle className="w-4 h-4 text-yellow-400 mt-0.5 shrink-0" />
                      <div className="flex-1">
                        <div className="text-sm font-medium text-white">{doc.documentName}</div>
                        <div className="text-xs text-slate-400 mt-1">
                          <span className="font-semibold">Reason:</span> {doc.reason}
                        </div>
                        {doc.ruleExpression && (
                          <div className="text-xs text-slate-500 mt-1 font-mono">
                            Rule: {doc.ruleExpression}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Generation History */}
      <div className="bg-slate-800/50 border border-slate-700/60 rounded-2xl p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-white flex items-center gap-2">
            <History className="w-5 h-5 text-purple-400" />
            Generation History
          </h3>
          <button
            onClick={loadData}
            className="text-sm text-slate-400 hover:text-white transition flex items-center gap-1"
          >
            <RefreshCw className="w-3 h-3" />
            Refresh
          </button>
        </div>

        {runs.length === 0 ? (
          <div className="text-center py-12 text-slate-500">
            <FileText className="w-12 h-12 mx-auto mb-3 opacity-50" />
            <p className="text-sm">No generation runs yet</p>
            <p className="text-xs mt-1">Click "Generate Documents" above to create your first run</p>
          </div>
        ) : (
          <div className="space-y-3">
            {runs.map((run) => (
              <div
                key={run.id}
                className={`rounded-lg p-4 border ${
                  selectedRun?.id === run.id
                    ? "bg-slate-700/50 border-blue-500"
                    : "bg-slate-900/50 border-slate-700 hover:bg-slate-800/50"
                } transition cursor-pointer`}
                onClick={() => setSelectedRun(selectedRun?.id === run.id ? null : run)}
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-2">
                      {getStatusIcon(run.status)}
                      <span className="text-sm font-semibold text-white">Run {run.id.slice(0, 8)}</span>
                      <span className={`px-2 py-0.5 rounded text-xs font-medium border ${getStatusColor(run.status)}`}>
                        {run.status.toUpperCase()}
                      </span>
                    </div>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs text-slate-400">
                      <div>
                        <span className="text-slate-500">Created:</span> {formatDate(run.createdAt)}
                      </div>
                      {run.completedAt && (
                        <div>
                          <span className="text-slate-500">Completed:</span> {formatDate(run.completedAt)}
                        </div>
                      )}
                      <div>
                        <span className="text-slate-500">Generated:</span> {run.documentCount} docs
                      </div>
                      {run.skippedCount > 0 && (
                        <div>
                          <span className="text-slate-500">Skipped:</span> {run.skippedCount} docs
                        </div>
                      )}
                    </div>
                    {run.errorMessage && (
                      <div className="mt-2 text-xs text-red-400">
                        <AlertCircle className="w-3 h-3 inline mr-1" />
                        {run.errorMessage}
                      </div>
                    )}
                  </div>
                  <div className="flex items-center gap-2 ml-4">
                    {run.status === "success" && (
                      <>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            handleRerun(run);
                          }}
                          className="p-2 text-slate-400 hover:text-blue-400 hover:bg-blue-500/10 rounded-lg transition"
                          title="Rerun"
                        >
                          <RefreshCw className="w-4 h-4" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            alert("Download functionality would be implemented here");
                          }}
                          className="p-2 text-slate-400 hover:text-green-400 hover:bg-green-500/10 rounded-lg transition"
                          title="Download"
                        >
                          <Download className="w-4 h-4" />
                        </button>
                      </>
                    )}
                  </div>
                </div>

                {/* Expanded run details */}
                {selectedRun?.id === run.id && run.status === "success" && (
                  <div className="mt-4 pt-4 border-t border-slate-700">
                    <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wide mb-2">
                      Generated Documents
                    </h4>
                    <div className="space-y-1">
                      {/* Placeholder for generated documents list */}
                      <div className="text-xs text-slate-500">
                        Document list would be fetched and displayed here
                      </div>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

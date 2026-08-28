import React from "react";
import { AlertTriangle } from "lucide-react";
import { logBug } from "../lib/ipc";

interface Props {
  children: React.ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches render-time exceptions so a single component failure does not blank
 * the entire application. Surfaces the error inline with a recovery action and
 * reports it to the Bug Book best-effort.
 */
export default class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // Best-effort capture into the Bug Book; never let reporting break the UI.
    logBug({
      errorType: "react_render_error",
      message: error.message,
      stackTrace: [error.stack, info.componentStack].filter(Boolean).join("\n"),
      severity: "high",
      context: "ErrorBoundary",
      category: "frontend",
      source: "auto",
    }).catch(() => {
      /* best-effort */
    });
  }

  handleReload = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-slate-900 p-6">
          <div className="max-w-md w-full bg-slate-800 border border-red-700/60 rounded-2xl p-6 shadow-2xl">
            <div className="flex items-center gap-3 text-red-400 mb-4">
              <AlertTriangle className="w-7 h-7 shrink-0" />
              <h2 className="text-lg font-bold text-white">Something went wrong</h2>
            </div>
            <p className="text-slate-300 text-sm mb-3">
              This screen hit an unexpected error. Your data is safe — you can retry.
            </p>
            <pre className="text-xs text-red-300 bg-slate-950 border border-slate-700 rounded p-3 mb-4 max-h-40 overflow-auto whitespace-pre-wrap">
              {this.state.error.message}
            </pre>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => window.location.reload()}
                className="px-4 py-2 text-slate-400 hover:text-white text-sm font-medium transition"
              >
                Reload App
              </button>
              <button
                onClick={this.handleReload}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white text-sm font-semibold rounded-xl transition"
              >
                Try Again
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

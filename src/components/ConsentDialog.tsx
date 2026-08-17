import React, { useEffect, useState } from "react";
import { Shield } from "lucide-react";
import { getTelemetryConsent } from "../lib/ipc";

interface Props {
  onConfirm: (optIn: boolean, crashReports: boolean) => void;
}

export default function ConsentDialog({ onConfirm }: Props) {
  const [optIn, setOptIn] = useState(false);
  const [crashReports, setCrashReports] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getTelemetryConsent()
      .then((state) => {
        setOptIn(state.optIn);
        setCrashReports(state.crashReports);
      })
      .catch(() => {
        /* best-effort default */
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  return (
    <div className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-6 max-w-md w-full shadow-2xl">
        <div className="flex items-center gap-3 mb-4">
          <Shield className="w-6 h-6 text-blue-400" />
          <h3 className="text-lg font-semibold text-white">Privacy & Telemetry Preferences</h3>
        </div>

        <p className="text-slate-400 text-sm mb-6">
          DocForge is designed privacy-first. We never transmit your document content or templates.
          Choose whether to share anonymous usage counts to help improve the software.
        </p>

        <div className="space-y-4 mb-6">
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={optIn}
              onChange={(e) => setOptIn(e.target.checked)}
              disabled={loading}
              className="mt-1 rounded bg-slate-800 border-slate-700 text-blue-600 focus:ring-blue-500"
            />
            <div>
              <span className="text-white text-sm font-medium">Anonymous Usage Analytics</span>
              <p className="text-slate-500 text-xs">Shares aggregate generation counts and timing.</p>
            </div>
          </label>

          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={crashReports}
              onChange={(e) => setCrashReports(e.target.checked)}
              disabled={loading}
              className="mt-1 rounded bg-slate-800 border-slate-700 text-blue-600 focus:ring-blue-500"
            />
            <div>
              <span className="text-white text-sm font-medium">Crash & Diagnostic Reports</span>
              <p className="text-slate-500 text-xs">Sends sanitized crash stacks if an engine error occurs.</p>
            </div>
          </label>
        </div>

        <div className="flex justify-end gap-3">
          <button
            onClick={() => onConfirm(false, false)}
            className="px-4 py-2 text-slate-400 hover:text-white text-sm transition"
          >
            Decline All
          </button>
          <button
            onClick={() => onConfirm(optIn, crashReports)}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded-lg text-sm transition"
          >
            Save Preferences
          </button>
        </div>
      </div>
    </div>
  );
}

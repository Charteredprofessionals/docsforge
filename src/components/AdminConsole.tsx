import React, { useState } from "react";
import { Users, Key, FileSpreadsheet, Settings } from "lucide-react";

export default function AdminConsole() {
  const [tab, setTab] = useState<"users" | "licenses" | "audit" | "policy">("users");

  return (
    <div className="max-w-6xl mx-auto p-6 text-white">
      <div className="flex items-center justify-between mb-8">
        <div>
          <h2 className="text-3xl font-bold">Admin Console</h2>
          <p className="text-slate-400 text-sm">Manage organization seats, licenses, audit logs, and security policies.</p>
        </div>
      </div>

      <div className="flex gap-4 border-b border-slate-800 mb-6">
        <button
          onClick={() => setTab("users")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "users" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <Users className="w-4 h-4" />
          Users & Seats
        </button>
        <button
          onClick={() => setTab("licenses")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "licenses" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <Key className="w-4 h-4" />
          Licensing
        </button>
        <button
          onClick={() => setTab("audit")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "audit" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <FileSpreadsheet className="w-4 h-4" />
          Audit Logs
        </button>
        <button
          onClick={() => setTab("policy")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "policy" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <Settings className="w-4 h-4" />
          Enterprise Policy
        </button>
      </div>

      <div className="bg-slate-900 border border-slate-800 rounded-xl p-6">
        {tab === "users" && (
          <div>
            <h3 className="text-lg font-semibold mb-2">Organization Seats</h3>
            <p className="text-slate-400 text-sm mb-4">Assign user roles: Viewer, Filler, Creator, Approver, Admin.</p>
            <div className="bg-slate-950 p-4 rounded-lg text-slate-400 text-sm">
              Local Admin (Owner) — <span className="text-green-400 font-mono">admin</span>
            </div>
          </div>
        )}
        {tab === "licenses" && (
          <div>
            <h3 className="text-lg font-semibold mb-2">Active License</h3>
            <p className="text-slate-400 text-sm mb-4">Tier: Free (Default)</p>
            <button className="bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg text-sm transition font-medium">
              Activate Offline License File (.dflic)
            </button>
          </div>
        )}
        {tab === "audit" && (
          <div>
            <h3 className="text-lg font-semibold mb-2">Immutable Audit Ledger</h3>
            <p className="text-slate-400 text-sm mb-4">Export view_audit_export table projection (REQ-013).</p>
            <button className="bg-slate-800 hover:bg-slate-700 text-white px-4 py-2 rounded-lg text-sm transition">
              Export Audit CSV / JSON
            </button>
          </div>
        )}
        {tab === "policy" && (
          <div>
            <h3 className="text-lg font-semibold mb-2">Policy Overrides</h3>
            <p className="text-slate-400 text-sm">Loaded from active policy_config ledger.</p>
          </div>
        )}
      </div>
    </div>
  );
}

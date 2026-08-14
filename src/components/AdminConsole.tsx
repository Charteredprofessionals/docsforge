import React, { useState } from "react";
import { Users, Key, FileSpreadsheet, Settings, Database, Download, Upload, Bug } from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { backupDatabase, restoreDatabase } from "../lib/ipc";
import BugBook from "./BugBook";

export default function AdminConsole() {
  const [tab, setTab] = useState<"users" | "licenses" | "audit" | "policy" | "data" | "bugs">("users");
  const [message, setMessage] = useState<string | null>(null);

  const handleBackup = async () => {
    setMessage(null);
    try {
      const path = await save({
        defaultPath: "docforge-backup.db",
        filters: [{ name: "SQLite DB", extensions: ["db"] }],
      });
      if (path) {
        await backupDatabase(path as string);
        setMessage("Backup created successfully.");
      }
    } catch (e) {
      setMessage(`Backup failed: ${e}`);
    }
  };

  const handleRestore = async () => {
    setMessage(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "SQLite DB", extensions: ["db"] }],
      });
      if (selected && typeof selected === "string") {
        await restoreDatabase(selected);
        setMessage("Database restored. Restart the app to apply changes.");
      }
    } catch (e) {
      setMessage(`Restore failed: ${e}`);
    }
  };

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
        <button
          onClick={() => setTab("data")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "data" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <Database className="w-4 h-4" />
          Data
        </button>
        <button
          onClick={() => setTab("bugs")}
          className={`flex items-center gap-2 pb-3 px-2 font-medium text-sm border-b-2 transition ${
            tab === "bugs" ? "border-blue-500 text-blue-400" : "border-transparent text-slate-400 hover:text-white"
          }`}
        >
          <Bug className="w-4 h-4" />
          Bug Book
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
        {tab === "data" && (
          <div>
            <h3 className="text-lg font-semibold mb-2">Database Management</h3>
            <p className="text-slate-400 text-sm mb-4">
              Back up or restore the local DocForge database (templates, audit logs, licenses).
            </p>
            <div className="flex gap-3 mb-4">
              <button
                onClick={handleBackup}
                className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg text-sm transition font-medium"
              >
                <Download className="w-4 h-4" /> Backup Database
              </button>
              <button
                onClick={handleRestore}
                className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 text-white px-4 py-2 rounded-lg text-sm transition"
              >
                <Upload className="w-4 h-4" /> Restore Database
              </button>
            </div>
            {message && <div className="text-slate-300 text-sm">{message}</div>}
          </div>
        )}
        {tab === "bugs" && <BugBook />}
      </div>
    </div>
  );
}

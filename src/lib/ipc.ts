import { invoke } from "@tauri-apps/api/core";

export interface ErrorResponse {
  code: string;
  message: string;
  detail?: Record<string, unknown>;
}

export class DocForgeError extends Error {
  code: string;
  detail?: Record<string, unknown>;

  constructor(res: ErrorResponse) {
    super(res.message);
    this.name = "DocForgeError";
    this.code = res.code;
    this.detail = res.detail;
  }
}

/**
 * Type-safe invoke wrapper that parses DocForgeError payloads into typed exceptions.
 */
export async function invokeApi<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    const rawResult = await invoke<string>(cmd, args);
    return JSON.parse(rawResult) as T;
  } catch (err) {
    if (typeof err === "string") {
      try {
        const parsed = JSON.parse(err) as ErrorResponse;
        if (parsed.code && parsed.message) {
          throw new DocForgeError(parsed);
        }
      } catch (parseErr) {
        if (parseErr instanceof DocForgeError) throw parseErr;
      }
      throw new Error(err);
    }
    throw err;
  }
}

/**
 * Transfer binary payloads across the Tauri bridge directly using Uint8Array.
 */
export async function invokeBinaryApi<T>(
  cmd: string,
  payload: Uint8Array,
  args?: Record<string, unknown>
): Promise<T> {
  return invokeApi<T>(cmd, { ...args, binaryData: Array.from(payload) });
}

export async function listTemplates(): Promise<import("./types").TemplateMeta[]> {
  return invokeApi<import("./types").TemplateMeta[]>("list_templates", {});
}

// ── Database backup / restore ─────────────────────────────────────────────────

export async function backupDatabase(targetPath: string): Promise<void> {
  return invokeApi<void>("backup_database", { targetPath });
}

export async function restoreDatabase(sourcePath: string): Promise<void> {
  return invokeApi<void>("restore_database", { sourcePath });
}

// ── Template Bundles ──────────────────────────────────────────────────────────

export interface CreateBundleInput {
  name: string;
  description?: string;
  templateIds: string[];
}

export async function createBundle(input: CreateBundleInput): Promise<string> {
  return invokeApi<string>("create_bundle_cmd", {
    name: input.name,
    description: input.description,
    templateIds: input.templateIds,
  });
}

export async function listBundles(): Promise<import("./types").Bundle[]> {
  return invokeApi<import("./types").Bundle[]>("list_bundles_cmd", {});
}

export async function getBundleTemplates(bundleId: string): Promise<string[]> {
  return invokeApi<string[]>("get_bundle_templates_cmd", { bundleId });
}

export async function deleteBundle(bundleId: string): Promise<void> {
  return invokeApi<void>("delete_bundle_cmd", { bundleId });
}

export async function addTemplateToBundle(
  bundleId: string,
  templateId: string
): Promise<void> {
  return invokeApi<void>("add_template_to_bundle_cmd", { bundleId, templateId });
}

export async function removeTemplateFromBundle(
  bundleId: string,
  templateId: string
): Promise<void> {
  return invokeApi<void>("remove_template_from_bundle_cmd", { bundleId, templateId });
}

// ── Bug Book (Admin Console crash/error log) ──────────────────────────────────

export interface LogBugInput {
  errorType: string;
  message: string;
  stackTrace?: string;
  severity?: "critical" | "high" | "medium" | "low";
  context?: string;
  category?: string;
  source?: "auto" | "manual";
}

export async function logBug(input: LogBugInput): Promise<string> {
  return invokeApi<string>("log_bug", {
    request: {
      errorType: input.errorType,
      message: input.message,
      stackTrace: input.stackTrace ?? "",
      severity: input.severity,
      context: input.context,
      category: input.category,
      source: input.source,
    },
  });
}

export interface CreateBugInput {
  errorType: string;
  message: string;
  severity: "critical" | "high" | "medium" | "low";
  status?: "open" | "in_progress" | "resolved" | "wont_fix";
  context?: string;
  stackTrace?: string;
  category?: string;
  keywords?: string;
}

export async function createBugEntry(input: CreateBugInput): Promise<string> {
  return invokeApi<string>("create_bug_entry", {
    request: {
      errorType: input.errorType,
      message: input.message,
      severity: input.severity,
      status: input.status,
      context: input.context,
      stackTrace: input.stackTrace,
      category: input.category,
      keywords: input.keywords,
    },
  });
}

export interface ListBugsInput {
  dateFrom?: string;
  dateTo?: string;
  severity?: string;
  status?: string;
  keyword?: string;
  sortBy?: string;
  sortDir?: string;
  limit?: number;
}

export async function listBugs(input: ListBugsInput): Promise<import("./types").BugEntry[]> {
  return invokeApi<import("./types").BugEntry[]>("list_bugs", {
    request: {
      dateFrom: input.dateFrom,
      dateTo: input.dateTo,
      severity: input.severity,
      status: input.status,
      keyword: input.keyword,
      sortBy: input.sortBy,
      sortDir: input.sortDir,
      limit: input.limit,
    },
  });
}

export async function getBug(bugId: string): Promise<import("./types").BugEntry> {
  return invokeApi<import("./types").BugEntry>("get_bug", { bugId });
}

export async function updateBugStatus(
  bugId: string,
  status: "open" | "in_progress" | "resolved" | "wont_fix",
  resolvedBy?: string
): Promise<void> {
  return invokeApi<void>("update_bug_status", { bugId, status, resolvedBy });
}

export interface AddBugAttachmentInput {
  bugId: string;
  filename: string;
  mimeType: string;
  dataB64: string;
}

export async function addBugAttachment(input: AddBugAttachmentInput): Promise<import("./types").BugAttachment> {
  return invokeApi<import("./types").BugAttachment>("add_bug_attachment", {
    bugId: input.bugId,
    filename: input.filename,
    mimeType: input.mimeType,
    dataB64: input.dataB64,
  });
}

export async function exportBugsCsv(input: ListBugsInput): Promise<string> {
  const res = await invokeApi<{ csv: string }>("export_bugs_csv", {
    request: {
      dateFrom: input.dateFrom,
      dateTo: input.dateTo,
      severity: input.severity,
      status: input.status,
      keyword: input.keyword,
      sortBy: input.sortBy,
      sortDir: input.sortDir,
      limit: input.limit,
    },
  });
  return res.csv;
}

export async function exportBugsPdf(input: ListBugsInput): Promise<{ pdfBase64: string; filename: string }> {
  return invokeApi<{ pdfBase64: string; filename: string }>("export_bugs_pdf", {
    request: {
      dateFrom: input.dateFrom,
      dateTo: input.dateTo,
      severity: input.severity,
      status: input.status,
      keyword: input.keyword,
      sortBy: input.sortBy,
      sortDir: input.sortDir,
      limit: input.limit,
    },
  });
}

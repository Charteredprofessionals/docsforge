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
    const message = err instanceof Error ? err.message : String(err);
    let code = "UNKNOWN_ERROR";
    let detail: Record<string, unknown> | undefined;

    try {
      const parsed = JSON.parse(message) as ErrorResponse;
      if (parsed.code && parsed.message) {
        code = parsed.code;
        detail = parsed.detail;
        throw new DocForgeError({ code, message: parsed.message, detail });
      }
    } catch {
      // Not a structured DocForgeError payload; fall through to generic Error
    }

    throw new Error(`${cmd}: ${message}`);
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

export async function getTemplate(templateId: string): Promise<import("./types").TemplateFull> {
  return invokeApi<import("./types").TemplateFull>("get_template", { templateId });
}

export async function uploadDocx(filePath: string): Promise<import("./types").UploadedDocx> {
  return invokeApi<import("./types").UploadedDocx>("upload_docx", { filePath });
}

export interface SaveTemplateInput {
  name: string;
  originalDocxB64: string;
  fields: import("./types").TemplateField[];
}

export async function saveTemplate(input: SaveTemplateInput): Promise<{ id: string; success: boolean }> {
  return invokeApi<{ id: string; success: boolean }>("save_template", {
    request: {
      name: input.name,
      original_docx_b64: input.originalDocxB64,
      fields: input.fields,
    },
  });
}

export async function deleteTemplate(templateId: string): Promise<void> {
  return invokeApi<void>("delete_template", { templateId });
}

export async function seedSampleTemplate(): Promise<{ id: string; already_exists?: boolean }> {
  return invokeApi<{ id: string; already_exists?: boolean }>("seed_sample_template", {});
}

export async function exportToPdf(docxBase64: string, outputFilename: string): Promise<{ pdfBase64: string; filename: string; engine: string }> {
  return invokeApi<{ pdfBase64: string; filename: string; engine: string }>("export_to_pdf", {
    request: {
      docx_base64: docxBase64,
      output_filename: outputFilename,
    },
  });
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
    request: {
      name: input.name,
      description: input.description,
      template_ids: input.templateIds,
    },
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

// ── Template field extraction / CSV export (mail-merge Phase A) ──────────────

export async function getTemplateFields(templateId: string): Promise<string[]> {
  return invokeApi<string[]>("get_template_fields", { request: { templateId } });
}

export async function exportTemplateFieldsCsv(templateId: string): Promise<string> {
  const res = await invokeApi<{ csv: string }>("export_template_fields_csv", {
    request: { templateId },
  });
  return res.csv;
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

// ── Fill Template ─────────────────────────────────────────────────────────────

export interface FillTemplateInput {
  templateId: string;
  values: Record<string, string>;
  replaceAll: boolean;
}

export async function fillTemplate(input: FillTemplateInput): Promise<string> {
  return invokeApi<string>("fill_template", {
    request: {
      templateId: input.templateId,
      values: input.values,
      replaceAll: input.replaceAll,
    },
  });
}

// ── Batch Fill from CSV (mail-merge Phase B) ─────────────────────────────────

export interface BatchFillFromCsvInput {
  templateId: string;
  csv: string;
  outputDir: string;
  formats: string[];
}

export interface BatchGeneratedFile {
  row: number;
  filename: string;
  path: string;
  sha256: string;
  status: string;
  error: string | null;
}

export interface BatchFillResult {
  generated: BatchGeneratedFile[];
  warnings: string[];
  errors: string[];
}

export async function batchFillFromCsv(input: BatchFillFromCsvInput): Promise<BatchFillResult> {
  return invokeApi<BatchFillResult>("batch_fill_from_csv", {
    request: {
      templateId: input.templateId,
      csv: input.csv,
      outputDir: input.outputDir,
      formats: input.formats,
    },
  });
}

// ── Telemetry & Consent ───────────────────────────────────────────────────────

export interface TelemetryConsentState {
  optIn: boolean;
  crashReports: boolean;
}

export async function getTelemetryConsent(): Promise<TelemetryConsentState> {
  return invokeApi<TelemetryConsentState>("get_telemetry_consent", {});
}

export async function setTelemetryConsent(input: TelemetryConsentState): Promise<void> {
  return invokeApi<void>("set_telemetry_consent", {
    request: {
      optIn: input.optIn,
      crashReports: input.crashReports,
    },
  });
}

// ── Current User / RBAC ───────────────────────────────────────────────────────

export interface CurrentUser {
  id: string;
  role: string;
  name: string;
  email: string;
}

export async function getCurrentUser(): Promise<CurrentUser> {
  return invokeApi<CurrentUser>("get_current_user", {});
}

export async function setUserRole(role: string): Promise<void> {
  return invokeApi<void>("set_user_role", { role });
}

// ── v2 Bundle APIs ────────────────────────────────────────────────────────────

export async function createBundleV2(name: string, description: string): Promise<string> {
  return invokeApi<string>("create_bundle_v2_cmd", { name, description });
}

export async function listBundlesV2(): Promise<import("./types").BundleSummary[]> {
  return invokeApi<import("./types").BundleSummary[]>("list_bundles_v2_cmd", {});
}

export async function getBundleV2(bundleId: string): Promise<import("./types").BundleDetail> {
  return invokeApi<import("./types").BundleDetail>("get_bundle_v2_cmd", { bundleId });
}

export async function publishVersion(bundleId: string, note: string): Promise<import("./types").BundleVersion> {
  return invokeApi<import("./types").BundleVersion>("publish_version_cmd", { bundleId, note });
}

export async function exportBundleDfpkg(bundleId: string, versionId: string): Promise<Uint8Array> {
  const result = await invokeApi<number[]>("export_bundle_dfpkg_cmd", { bundleId, versionId });
  return new Uint8Array(result);
}

export async function importBundleDfpkg(dfpkgBytes: Uint8Array): Promise<string> {
  return invokeBinaryApi<string>("import_bundle_dfpkg_cmd", dfpkgBytes);
}

export async function listVersions(bundleId: string): Promise<import("./types").BundleVersion[]> {
  return invokeApi<import("./types").BundleVersion[]>("list_versions_cmd", { bundleId });
}

// ── v2 Field APIs ─────────────────────────────────────────────────────────────

export async function createField(bundleId: string, field: Partial<import("./types").FieldDef>): Promise<string> {
  return invokeApi<string>("create_field_cmd", { bundleId, field });
}

export async function listFields(bundleId: string): Promise<import("./types").FieldDef[]> {
  return invokeApi<import("./types").FieldDef[]>("list_fields_cmd", { bundleId });
}

export async function setMapping(
  documentId: string,
  placeholder: string,
  fieldId: string,
  transformExpr?: string
): Promise<void> {
  return invokeApi<void>("set_mapping_cmd", { documentId, placeholder, fieldId, transformExpr });
}

export async function listMappings(documentId: string): Promise<import("./types").FieldMapping[]> {
  return invokeApi<import("./types").FieldMapping[]>("list_mappings_cmd", { documentId });
}

export async function findUnmappedPlaceholders(bundleId: string): Promise<import("./types").UnmappedPlaceholder[]> {
  return invokeApi<import("./types").UnmappedPlaceholder[]>("find_unmapped_placeholders_cmd", { bundleId });
}

// ── Matter APIs ───────────────────────────────────────────────────────────────

export async function createMatter(bundleId: string, name: string): Promise<string> {
  return invokeApi<string>("create_matter_cmd", { bundleId, name });
}

export async function listMatters(bundleId?: string): Promise<import("./types").MatterMeta[]> {
  return invokeApi<import("./types").MatterMeta[]>("list_matters_cmd", { bundleId });
}

export async function getMatter(matterId: string): Promise<import("./types").MatterMeta> {
  return invokeApi<import("./types").MatterMeta>("get_matter_cmd", { matterId });
}

export async function renderMatterForm(matterId: string): Promise<import("./types").MatterForm> {
  return invokeApi<import("./types").MatterForm>("render_matter_form_cmd", { matterId });
}

export async function setMatterValue(matterId: string, fieldId: string, value: unknown): Promise<void> {
  return invokeApi<void>("set_matter_value_cmd", { matterId, fieldId, value });
}

export async function getMatterValue(matterId: string, fieldId: string): Promise<unknown> {
  return invokeApi<unknown>("get_matter_value_cmd", { matterId, fieldId });
}

export async function validateMatter(matterId: string): Promise<import("./types").ValidationReport> {
  return invokeApi<import("./types").ValidationReport>("validate_matter_cmd", { matterId });
}

export async function updateMatterStatus(
  matterId: string,
  status: "draft" | "ready" | "generated" | "archived"
): Promise<void> {
  return invokeApi<void>("update_matter_status_cmd", { matterId, status });
}

export async function deleteMatter(matterId: string): Promise<void> {
  return invokeApi<void>("delete_matter_cmd", { matterId });
}

// ── Generation APIs ───────────────────────────────────────────────────────────

export async function listGenerationRuns(matterId: string): Promise<import("./types").GenerationRun[]> {
  return invokeApi<import("./types").GenerationRun[]>("list_runs_cmd", { matterId });
}

export async function evaluatePreview(matterId: string, documentIds?: string[]): Promise<import("./types").GenerationPreview> {
  return invokeApi<import("./types").GenerationPreview>("evaluate_preview_cmd", { matterId, documentIds });
}

export async function executeRun(matterId: string, documentIds?: string[]): Promise<import("./types").ExecuteResult> {
  return invokeApi<import("./types").ExecuteResult>("execute_run_cmd", { matterId, documentIds });
}

export async function getRun(runId: string): Promise<import("./types").GenerationRun> {
  return invokeApi<import("./types").GenerationRun>("get_run_cmd", { runId });
}

// ── Rules APIs ────────────────────────────────────────────────────────────────

export async function addRule(documentId: string, expression: string, description?: string): Promise<string> {
  return invokeApi<string>("add_rule_cmd", { documentId, expression, description });
}

export async function removeRule(ruleId: string): Promise<void> {
  return invokeApi<void>("remove_rule_cmd", { ruleId });
}

export async function listRules(documentId: string): Promise<import("./types").Rule[]> {
  return invokeApi<import("./types").Rule[]>("list_rules_cmd", { documentId });
}

export async function evaluateRules(matterId: string): Promise<import("./types").DocumentDecision[]> {
  return invokeApi<import("./types").DocumentDecision[]>("evaluate_rules_cmd", { matterId });
}

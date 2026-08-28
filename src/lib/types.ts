export type FieldKind = "text" | "date" | "dropdown" | "checkbox" | "signature";

export interface TemplateField {
  id: string;
  label: string;
  originalText: string;
  tagName: string;
  fieldType?: FieldKind;
  options?: string[];
  required?: boolean;
}

export type TemplateStatus = "draft" | "review" | "published" | "archived";

export interface TemplateMeta {
  id: string;
  org_id?: string;
  name: string;
  category: string;
  description: string;
  current_version: number;
  status: TemplateStatus;
  storage_path: String;
  fields_json: string;
  content_sha256: string;
  created_at: string;
  updated_at: string;
}

export interface TemplateFull {
  id: string;
  name: string;
  category: string;
  description: string;
  version: number;
  status: TemplateStatus;
  fields: TemplateField[];
  template_docx_b64: string;
  created_at: string;
}

export interface UploadedDocx {
  filename: string;
  base64?: string;
  textContent?: string;
}

export interface Bundle {
  id: string;
  name: string;
  description: string;
  created_at: string;
}

export type BugSeverity = "critical" | "high" | "medium" | "low";
export type BugStatus = "open" | "in_progress" | "resolved" | "wont_fix";

export interface BugAttachment {
  id: string;
  bugId: string;
  filename: string;
  mimeType: string;
  createdAt: string;
}

export interface BugEntry {
  id: string;
  createdAt: string;
  updatedAt: string;
  errorType: string;
  severity: BugSeverity;
  status: BugStatus;
  context: string;
  message: string;
  stackTrace: string;
  source: "auto" | "manual";
  category: string;
  keywords: string;
  resolvedBy?: string | null;
  resolvedAt?: string | null;
  attachments: BugAttachment[];
}

export interface BugFilter {
  dateFrom?: string;
  dateTo?: string;
  severity?: BugSeverity | "";
  status?: BugStatus | "";
  keyword?: string;
  sortBy?: "createdAt" | "severity" | "status" | "errorType";
  sortDir?: "asc" | "desc";
  limit?: number;
}

export type AppView = "list" | "create" | "fill" | "admin" | "bundles" | "dashboard" | "matters" | "matter-form" | "generation-history" | "bundle-detail";

// ── v2 Bundle Types ──────────────────────────────────────────────────────────

export interface BundleVersion {
  id: string;
  bundleId: string;
  versionNumber: number;
  note: string;
  status: "draft" | "review" | "published" | "archived";
  createdAt: string;
  publishedAt?: string;
}

export interface BundleDocument {
  id: string;
  bundleId: string;
  templateId: string;
  name: string;
  displayOrder: number;
}

export interface BundleSummary {
  id: string;
  name: string;
  description: string;
  status: "draft" | "published" | "archived";
  versionCount: number;
  currentVersion?: number;
  createdAt: string;
  updatedAt: string;
}

export interface BundleDetail extends BundleSummary {
  documents: BundleDocument[];
  versions: BundleVersion[];
  unmappedPlaceholders: UnmappedPlaceholder[];
}

export interface UnmappedPlaceholder {
  documentId: string;
  documentName: string;
  placeholder: string;
  count: number;
}

// ── v2 Field Types ───────────────────────────────────────────────────────────

export type FieldType = 
  | "text" 
  | "multiline_text" 
  | "number" 
  | "currency" 
  | "percentage" 
  | "date" 
  | "datetime" 
  | "boolean" 
  | "email" 
  | "phone" 
  | "url" 
  | "select" 
  | "multiselect";

export interface FieldDef {
  id: string;
  name: string;
  label: string;
  fieldType: FieldType;
  required: boolean;
  defaultValue?: string;
  options?: string[];
  placeholder?: string;
  helpText?: string;
  groupId?: string;
}

export interface FieldGroup {
  id: string;
  name: string;
  label: string;
  scope: "shared" | "document";
  documentId?: string;
  displayOrder: number;
}

export interface FieldMapping {
  id: string;
  documentId: string;
  placeholder: string;
  fieldId: string;
  transformExpr?: string;
}

// ── Matter Types ─────────────────────────────────────────────────────────────

export interface MatterMeta {
  id: string;
  name: string;
  bundleId: string;
  bundleVersionId: string;
  status: "draft" | "ready" | "generated" | "archived";
  createdAt: string;
  updatedAt: string;
}

export interface FormField {
  id: string;
  fieldDef: FieldDef;
  value?: unknown;
  error?: string;
}

export interface FormGroup {
  group: FieldGroup;
  fields: FormField[];
}

export interface MatterForm {
  matterId: string;
  matterName: string;
  bundleName: string;
  groups: FormGroup[];
}

export interface ValidationError {
  fieldId: string;
  fieldName: string;
  error: string;
}

export interface ValidationReport {
  valid: boolean;
  errors: ValidationError[];
  warnings: string[];
}

// ── Generation Types ─────────────────────────────────────────────────────────

export interface GenerationRun {
  id: string;
  matterId: string;
  bundleVersionId: string;
  status: "pending" | "running" | "success" | "failed";
  documentCount: number;
  skippedCount: number;
  createdAt: string;
  completedAt?: string;
  errorMessage?: string;
}

export interface SkippedDocument {
  documentId: string;
  documentName: string;
  reason: string;
  ruleExpression?: string;
}

export interface GenerationPreview {
  totalDocuments: number;
  includedCount: number;
  skippedCount: number;
  skipped: SkippedDocument[];
}

export interface GeneratedDocument {
  id: string;
  runId: string;
  documentId: string;
  documentName: string;
  outputPath: string;
  sha256: string;
  format: "docx" | "pdf";
  createdAt: string;
}

export interface ExecuteResult {
  runId: string;
  status: "success" | "partial" | "failed";
  generated: GeneratedDocument[];
  errors: string[];
}

// ── Rules Types ──────────────────────────────────────────────────────────────

export interface Rule {
  id: string;
  documentId: string;
  expression: string;
  description?: string;
  createdAt: string;
}

export interface DocumentDecision {
  documentId: string;
  documentName: string;
  decision: "include" | "exclude";
  reason: string;
}

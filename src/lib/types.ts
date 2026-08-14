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

export type AppView = "list" | "create" | "fill" | "admin" | "bundles";

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

export type AppView = "list" | "create" | "fill";

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

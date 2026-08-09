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

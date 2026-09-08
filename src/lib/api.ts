import { invoke, isTauri } from "@tauri-apps/api/core";
import { snapshotSchema, type Action } from "./contracts";

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}
export const desktop = isTauri();
async function request(path: string, body?: unknown) {
  const response = await fetch(`/api/v1${path}`, {
    method: body ? "POST" : "GET",
    credentials: "same-origin",
    headers: body
      ? { "Content-Type": "application/json", "x-kairos-command": "1" }
      : {},
    body: body ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(90_000),
  });
  const data: unknown = await response
    .json()
    .catch(() => ({ error: "The runtime returned an invalid response" }));
  if (!response.ok)
    throw new ApiError(
      typeof data === "object" && data !== null && "error" in data
        ? String(data.error)
        : `Request failed (${response.status})`,
      response.status,
    );
  return data;
}
export async function getSnapshot() {
  return snapshotSchema.parse(
    desktop ? await invoke("get_runtime_status") : await request("/runtime"),
  );
}
export async function sendCommand(action: Action) {
  const command = { ...action, request_id: crypto.randomUUID() };
  try {
    return snapshotSchema.parse(
      desktop
        ? await invoke("runtime_command", { command })
        : await request("/commands", command),
    );
  } catch (error) {
    throw error instanceof Error ? error : new Error(String(error));
  }
}
export async function login(token: string) {
  await request("/auth/login", { token });
}
export async function logout() {
  await request("/auth/logout", {});
}

import type { Snapshot } from "./types";
export const isDemo = new URLSearchParams(location.search).get("demo") === "1";
export async function api<T = unknown>(
  method: string,
  params: unknown = {},
): Promise<T> {
  if (isDemo) {
    const { demoApi } = await import("./demo");
    return demoApi(method, params) as T;
  }
  if ("__TAURI_INTERNALS__" in window) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>("api", { method, params });
  }
  const response = await fetch("/api", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method, params }),
  });
  if (!response.headers.get("content-type")?.includes("application/json"))
    throw new Error("本地服务未连接，请使用 pnpm dev 启动完整应用。");
  const body = await response.json();
  if (body.error) throw new Error(body.error);
  return body.result;
}
export const snapshot = () => api<Snapshot>("snapshot");

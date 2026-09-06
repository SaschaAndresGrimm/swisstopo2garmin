// Thin wrapper over the Tauri IPC surface. Types come from bindings.ts, which is
// generated from the Rust definitions (see src-tauri/src/ipc.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CacheStatus, ReleaseInfo, TaskDone, TaskError, TaskProgress } from "./bindings";

export type { CacheStatus, DatasetEntry, ReleaseInfo, TaskProgress } from "./bindings";

export const TLM3D = "ch.swisstopo.swisstlm3d";
export const WANDERWEGE = "ch.swisstopo.swisstlm3d-wanderwege";

/** Sources the Data screen manages, in the order they are shown. */
export const SOURCES = [
  { id: TLM3D, key: "source.tlm3d" },
  { id: WANDERWEGE, key: "source.wanderwege" },
] as const;

export const api = {
  cacheStatus: () => invoke<CacheStatus>("cache_status"),
  latestRelease: (collection: string) => invoke<ReleaseInfo>("latest_release", { collection }),
  acquire: (collection: string) => invoke<string>("acquire_dataset", { collection }),
  cancel: (taskId: string) => invoke<void>("cancel_task", { taskId }),
  remove: (collection: string, item: string) => invoke<void>("remove_dataset", { collection, item }),
};

/**
 * Subscribe to task events.
 *
 * `onFailure` matters: Tauri 2 denies `listen()` unless the window has an event
 * capability, and the rejection is otherwise silent — the UI simply never updates
 * while `invoke` keeps working, which looks like a frontend bug. Surface it.
 */
export function onTaskEvents(handlers: {
  progress?: (p: TaskProgress) => void;
  done?: (d: TaskDone) => void;
  error?: (e: TaskError) => void;
  onFailure?: (reason: string) => void;
}): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<TaskProgress>("task:progress", (e) => handlers.progress?.(e.payload)),
    listen<TaskDone>("task:done", (e) => handlers.done?.(e.payload)),
    listen<TaskError>("task:error", (e) => handlers.error?.(e.payload)),
  ]).catch((reason) => {
    const msg = `cannot subscribe to task events: ${String(reason)}`;
    console.error(msg);
    handlers.onFailure?.(msg);
    return [] as UnlistenFn[];
  });
}

export function formatBytes(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

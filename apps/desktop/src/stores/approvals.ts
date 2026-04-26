import { createResource, createRoot } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface PendingRun {
  id: number;
  automationId: number;
  automationName: string;
  agentId: string;
  queuedAt: number;
  prompt: string;
}

function createApprovalStore() {
  const [pending, { refetch: refetchPending }] = createResource(() =>
    invoke<PendingRun[]>("list_pending_approvals"),
  );
  const [count, { refetch: refetchCount }] = createResource(() =>
    invoke<number>("count_pending_approvals"),
  );

  const refresh = () => {
    void refetchPending();
    void refetchCount();
  };

  void listen("approvals:changed", refresh);
  // Cron / webhook / event triggers fire approval rows too — `automation:fired`
  // is emitted on completion of a run, not when one queues. Until the
  // backend grows a dedicated `approvals:queued` event, the periodic
  // resource refetch (every read on this store) keeps the badge fresh.
  void listen("automation:fired", refresh);

  return { pending, count, refetchPending, refetchCount };
}

export const { pending, count, refetchPending, refetchCount } = createRoot(
  createApprovalStore,
);

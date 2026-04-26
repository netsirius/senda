import { createResource, createRoot } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import type { CatalogEntry } from "senda-shared-types";

async function fetchCatalog(): Promise<CatalogEntry[]> {
  return await invoke<CatalogEntry[]>("read_catalog");
}

/**
 * Single shared resource — every page that needs the catalog reads from the
 * same `catalog()` accessor and shares the same fetch. `refetch()` triggers
 * a re-read from disk; subscribers re-render automatically.
 */
function createCatalogStore() {
  const [catalog, { refetch, mutate }] = createResource(fetchCatalog);
  return { catalog, refetch, mutate };
}

export const { catalog, refetch: refetchCatalog, mutate: mutateCatalog } = createRoot(
  createCatalogStore,
);

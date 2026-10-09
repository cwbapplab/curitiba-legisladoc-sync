import { invoke } from "@tauri-apps/api/core";

/* ------------------------------------------------------------- act data ---- */

export interface ActRow {
  id: number;
  numero_identificacao: string | null;
  numero_oficial: string | null;
  ano_publicacao: number | null;
  data_publicacao: string | null;
  tipo: string | null;
  orgao: string | null;
  situacao: string | null;
  sumula: string | null;
  /** Matching excerpt with `<mark>` tags, present only for full-text queries. */
  snippet: string | null;
}

export interface FieldValue {
  column: string;
  label: string;
  value: string | null;
  related_value: string | null;
  related_table: string | null;
}

export interface ActDetail {
  fields: FieldValue[];
  body: string | null;
}

export interface Facet {
  value: string;
  count: number;
}

export interface StoreStats {
  total: number;
  with_body: number;
  newest_publication: string | null;
  oldest_publication: string | null;
  newest_created: string | null;
}

export interface SearchResult {
  rows: ActRow[];
  total: number;
}

/** The filter operators the backend enforces. */
export type Op = "eq" | "ne" | "like" | "gte" | "lte";

export interface Filter {
  column: string;
  op: Op;
  value: string;
}

export interface ActQuery {
  text?: string | null;
  filters: Filter[];
  sort?: string | null;
  ascending: boolean;
  limit: number;
  offset: number;
}

/**
 * The columns the UI may filter and sort on, and the labels `Info.xml` gives
 * them. Fetched rather than hard-coded so the controls always match what the
 * database will actually accept.
 */
export interface QueryCapabilities {
  filters: string[];
  sorts: string[];
  labels: [string, string][];
}

export interface SyncReport {
  snapshot: string;
  action: "unchanged" | "appended" | "reloaded" | string;
  rows_added: number;
  total_rows: number;
  dimensions: number;
  bytes_fetched: number;
  detail: string | null;
}

/* -------------------------------------------------- rsync explorer page ---- */

export interface FileRow {
  id: number;
  path: string;
  size: number;
  mtime: string | null;
  /** Changed within the trailing 7-day window. */
  recent: boolean;
}

export interface Summary {
  total_files: number;
  total_bytes: number;
  /** Bytes of every file the server listed, including filtered-out entries. */
  listed_bytes: number;
  newest_mtime: string | null;
  recent_count: number;
  added_last_sync: number;
  removed_last_sync: number;
}

export interface LastSync {
  at: string;
  total: number;
  added: number;
  updated: number;
  removed: number;
  error: string | null;
}

export interface AppSnapshot {
  summary: Summary;
  last_sync: LastSync | null;
  syncing: boolean;
  source: string;
  interval_minutes: number;
  last_error: string | null;
  act_report: SyncReport | null;
}

export interface SearchPage {
  rows: FileRow[];
  total: number;
}

export type SortBy = "mtime" | "size" | "path" | "added";

/* -------------------------------------------------------------- commands --- */

export const getState = () => invoke<AppSnapshot>("get_state");
export const syncNow = () => invoke<SyncReport>("sync_now");
export const setIntervalMinutes = (minutes: number) =>
  invoke<void>("set_interval", { minutes });

export const actStats = () => invoke<StoreStats>("act_stats");
export const searchActs = (query: ActQuery) =>
  invoke<SearchResult>("search_acts", { query });
export const actDetail = (id: number) =>
  invoke<ActDetail | null>("act_detail", { id });
export const actFacets = (column: string, limit = 40) =>
  invoke<Facet[]>("act_facets", { column, limit });
export const queryCapabilities = () =>
  invoke<QueryCapabilities>("query_capabilities");

export const searchFiles = (
  query: string,
  sortBy: SortBy,
  ascending: boolean,
  limit = 500,
  offset = 0
) => invoke<SearchPage>("search_files", { query, sortBy, ascending, limit, offset });

/* ------------------------------------------------------------- formatting -- */

/** 1_234_567 -> "1.2 MB" (decimal units, matching how file managers label sizes). */
export function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    Math.floor(Math.log10(Math.abs(bytes)) / 3),
    units.length - 1
  );
  const value = bytes / 1000 ** i;
  // Big values need no decimal; bytes read wrong with one.
  return `${value.toFixed(value >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Compact form for a hero tile: 12,990 -> "12.9K". */
export function compactNumber(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}K`;
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

/**
 * Feed timestamps -> "2026-10-08" in the user's local time.
 *
 * Values look like `2019-11-05 00:00:00.000`: no timezone, so `new Date()` would
 * read them as UTC and shift the day for anyone west of Greenwich. The date part
 * is already what the act view shows, so it is used verbatim rather than parsed.
 */
export function formatActDate(value: string | null): string {
  if (!value) return "—";
  const date = value.slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}$/.test(date) ? date : value;
}

/** RFC 3339 -> "2026-10-08 08:27" in the user's local time. */
export function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours()
  )}:${pad(d.getMinutes())}`;
}

/** "há 3 minutos", para a linha de estado. */
export function relativeTime(iso: string | null): string {
  if (!iso) return "nunca";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "nunca";
  const secs = Math.max(0, Math.floor((Date.now() - then) / 1000));
  if (secs < 60) return "agora mesmo";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `há ${mins} minuto${mins === 1 ? "" : "s"}`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `há ${hours} hora${hours === 1 ? "" : "s"}`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `há ${days} dia${days === 1 ? "" : "s"}`;
  const months = Math.floor(days / 30);
  if (months < 12) return `há ${months} ${months === 1 ? "mês" : "meses"}`;
  const years = Math.floor(days / 365);
  return `há ${years} ano${years === 1 ? "" : "s"}`;
}

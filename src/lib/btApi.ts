// BT 磁力下載 IPC 封裝 + TS 契約（後端 torrent/ 模組）

import { invoke } from "@tauri-apps/api/core";

// ---- Event 契約（後端 torrent/events.rs） ----

export type TorrentState = "initializing" | "live" | "paused" | "error";

export interface TorrentStatsItem {
  id: number;
  name: string | null; // metadata 抓到前為 null
  state: TorrentState;
  finished: boolean;
  progress_percent: number; // 0-100
  downloaded_bytes: number;
  total_bytes: number;
  down_speed_bps: number;
  up_speed_bps: number;
  peers_live: number;
  error: string | null;
}

export interface PendingItem {
  key: number;
  name: string | null; // magnet dn=，可能沒有
  elapsed_s: number;
  error: string | null; // 背景 add 失敗時有值
}

export interface TorrentStatsEvent {
  torrents: TorrentStatsItem[];
  pending: PendingItem[];
  session: { total_down_bps: number; total_up_bps: number };
}

export interface TorrentFinishedEvent {
  id: number;
  name: string | null;
}

// ---- Command payloads（librqbit serializable types 直通） ----

export interface TorrentFileDetails {
  name: string;
  components: string[];
  length: number;
  included: boolean;
}

export interface TorrentDetails {
  id: number | null;
  info_hash: string;
  name: string | null;
  output_folder: string;
  files?: TorrentFileDetails[];
}

export interface AddMagnetResult {
  already_exists?: boolean;
  pending?: boolean; // add 在背景跑，透過 stats event 的 pending 清單追蹤
  key?: number;
  id?: number | null;
  name?: string | null;
}

// ---- Command wrappers ----
// BtSettings 契約與 get/save 已併入 settingsApi.ts（AppSettings.bt）

export function addMagnet(
  magnet: string,
  outDir?: string,
  paused = false,
): Promise<AddMagnetResult> {
  return invoke("add_magnet", { magnet, outDir: outDir ?? null, paused });
}

export function removePending(key: number): Promise<void> {
  return invoke("remove_pending", { key });
}

export function torrentDetails(id: number): Promise<TorrentDetails> {
  return invoke("torrent_details", { id });
}

export function pauseTorrent(id: number): Promise<void> {
  return invoke("pause_torrent", { id });
}

export function resumeTorrent(id: number): Promise<void> {
  return invoke("resume_torrent", { id });
}

export function deleteTorrent(id: number, deleteFiles: boolean): Promise<void> {
  return invoke("delete_torrent", { id, deleteFiles });
}

// ---- 引擎狀態（背景 init，失敗可 retry） ----

export interface BtEngineStatus {
  ready: boolean;
  error: string | null;
}

export function getBtEngineStatus(): Promise<BtEngineStatus> {
  return invoke("get_bt_engine_status");
}

export function retryBtInit(): Promise<void> {
  return invoke("retry_bt_init");
}

// 工作需求遊戲設定批次改寫 IPC 封裝 + TS 契約（後端 jin/ 模組）

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type JinAction = "add" | "add_commented" | "uncomment" | "skip" | "error";

export interface JinFilePlan {
  path: string;
  env: string;
  action: JinAction;
  message: string;
  applied: boolean;
}

export interface JinPreview {
  hall: string;
  codes: string[];
  roots: string[];
  files: JinFilePlan[];
}

/** 後端逐檔推的進度（phase "done" = 結束,前端收掉進度條） */
export interface JinProgress {
  phase: "scan" | "write" | "done";
  done: number;
  total: number;
  path: string;
}

export function listenJinProgress(cb: (p: JinProgress) => void): Promise<UnlistenFn> {
  return listen<JinProgress>("jin-progress", (e) => cb(e.payload));
}

export interface JinParams {
  /** 空陣列 = 用 app_settings.json 的 jin_roots */
  roots: string[];
  hall: string;
  suffixes: string[];
  commentEnvs: string[];
}

export function jinPreview(p: JinParams): Promise<JinPreview> {
  return invoke("jin_preview", {
    roots: p.roots,
    hall: p.hall,
    suffixes: p.suffixes,
    commentEnvs: p.commentEnvs,
  });
}

export function jinApply(p: JinParams): Promise<JinFilePlan[]> {
  return invoke("jin_apply", {
    roots: p.roots,
    hall: p.hall,
    suffixes: p.suffixes,
    commentEnvs: p.commentEnvs,
  });
}

/** "E F C" / "E,F,C" → ["E","F","C"] */
export function parseSuffixes(raw: string): string[] {
  return raw
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

export const ACTION_LABEL: Record<JinAction, string> = {
  add: "新增",
  add_commented: "新增（註解）",
  uncomment: "解除註解",
  skip: "略過",
  error: "錯誤",
};

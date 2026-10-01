// src/hooks/useTaskEvents.ts

import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ClipboardPayload, IngestSummary, SeriesStartEvent } from '../types';

type AddTaskFunction = (payload: ClipboardPayload, opts?: { silent?: boolean }) => Promise<void>;

function seriesSummaryText(s: IngestSummary): string {
    const parts = [`新增 ${s.added} 話`];
    if (s.existed) parts.push(`已在清單 ${s.existed} 話`);
    if (s.skipped) parts.push(`已下載過 ${s.skipped} 話`);
    if (s.failed) parts.push(`失敗 ${s.failed} 話（再加入一次可補抓）`);
    return `合集《${s.series_title ?? ''}》:${parts.join('，')}`;
}

/**
 * useTaskEvents — 網站下載任務進清單的唯一入口（後端 `ingest.rs` 的事件）
 * - 剪貼簿與拖曳共用：後端一律 emit，所以這裡**不受監控開關控制**
 *   （監控關掉時後端本來就不會從剪貼簿 emit；以前 gate 住反而讓「關開關當下還在抓的那筆」
 *   進了 DB 卻沒進畫面）
 * - 合集：每話 `series-item-added` 靜音加入，`series-done` 出一則統計 toast + 整批一次 ding
 *   （不然 30 話叮 30 次）
 */
export const useTaskEvents = (
    addTask: AddTaskFunction,
    pushToast: (text: string) => void,
    playDing: () => void,
): void => {
    useEffect(() => {
        let mounted = true;
        const unlisteners: (() => void)[] = [];
        const track = (p: Promise<() => void>) =>
            p.then(fn => (mounted ? unlisteners.push(fn) : fn()));

        track(listen<ClipboardPayload>('new-valid-url-payload', e => {
            if (mounted) addTask(e.payload);
        }));

        track(listen<SeriesStartEvent>('series-start', e => {
            if (mounted) pushToast(`合集《${e.payload.title}》共 ${e.payload.total} 話，逐話抓取中…`);
        }));

        track(listen<ClipboardPayload>('series-item-added', e => {
            if (mounted) addTask(e.payload, { silent: true });
        }));

        track(listen<IngestSummary>('series-done', e => {
            if (!mounted) return;
            if (e.payload.added > 0) playDing();
            pushToast(seriesSummaryText(e.payload));
        }));

        // 抓取失敗（網路掛了/站台改版/provider 未實作）也要讓使用者知道，
        // 否則畫面上跟「沒複製到」完全一樣。與 magnet-add-error 對稱。
        track(listen<string>('url-fetch-error', e => {
            if (mounted) pushToast(`連結處理失敗:${e.payload}`);
        }));

        return () => {
            mounted = false;
            unlisteners.forEach(fn => fn());
        };
    }, [addTask, pushToast, playDing]);
};

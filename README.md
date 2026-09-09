# tauri-downloader

Tauri 2.0 + React 19 的個人下載工具（Windows）。監控剪貼簿，複製到支援的連結就自動建任務；四個分頁各管一種來源。

| 分頁 | 做什麼 |
|------|--------|
| 網站下載 | 站台爬取（目前只有 wnacg）：抓標題/封面/檔案連結 → 下載 zip。清單存 SQLite，可拖曳排序、批次下載、斷點續傳 |
| 磁力下載 | BT（librqbit 引擎）。複製 magnet 自動加入，可暫停/恢復/刪除，重開 app 接續 |
| 直鏈下載 | 任意 HTTP(S) 直鏈。最多 4 段並行 + work stealing、斷點續傳、連結過期可換新連結接著下 |
| 遊戲設定 | 工作用：批次改寫 jinbaba 的 `code.*.php`（先預覽每個檔會怎麼改，確認後才寫檔，**不備份**） |

網站下載與直鏈下載共用同一套下載引擎（`src-tauri/src/dl.rs`）：Range 探測、分段並行、限速、`.part` 暫存檔、完成才 rename。

## 開發環境（Windows）

需要 Rust toolchain（MSVC）＋ Node（用 nvm 或 pnpm 自帶的 env）。

```powershell
iwr https://get.pnpm.io/install.ps1 -useb | iex
pnpm env use --global lts
pnpm install
```

> 早期試過用 docker 開發環境，Tauri 要 GUI/WebView 太麻煩，改用 nvm 直接裝在本機。

## 指令

```bash
pnpm tauri dev               # 開發模式跑完整 app
pnpm tauri build             # 正式建置
pnpm tauri icon app-icon.svg # 從根目錄 SVG 重產全尺寸 icon
cd src-tauri && cargo test   # Rust 單元測試
npx tsc --noEmit             # 前端型別檢查
```

前端 dev server 固定 port 1420（`vite.config.ts` 的 `strictPort`）。

## 剪貼簿監控流程（網站下載）

1. 背景執行緒每 500ms 讀一次剪貼簿（`src-tauri/src/monitor.rs`），內容有變才處理，同一個連結 30 秒內不重複抓。
2. `magnet:` 開頭 → 交給 BT 引擎；否則 `Site::from_url()` 認站台、`validate()` 驗路徑並把 URL 規範化（丟掉 query、host 統一），同一部作品的各種寫法收斂成同一筆。
3. `fetch_details()` 抓作品頁：標題（`#bodywrap > h2`）、封面（`.pic_box img`）、下載頁連結（`#ads > a`），順便預取真正的 zip 連結並用 `Range: bytes=0-0` 探測檔案大小（預檢 404/410 直接標 `not_found`）。
4. 下載目錄裡已經有同名成品就略過；否則寫進 SQLite 並 emit `new-valid-url-payload`，前端把它加進清單、播提示音。

抓取失敗會 emit `url-fetch-error` 顯示 toast，並把該連結移出節流名單（可以立刻再複製一次重試）。

拖曳連結到視窗也能新增（走 `add_url_manually`，與剪貼簿同一條 pipeline，不受監控開關影響）。

## 設定與資料位置

所有後端設定收在統一設定 dialog（tab bar 的 ⚙），存 `app_data_dir/app_settings.json`；純 UI 偏好（主題、分頁、音量、欄寬）留在 localStorage。

`app_data_dir` 在 Windows 是 `%APPDATA%\com.kawa.tauri-downloader\`：

| 檔案 | 內容 |
|------|------|
| `app_settings.json` | 監控開關、三個下載分頁的預設目錄與限速、BT port、jin 根目錄 |
| `tasks.db` | 網站下載任務（SQLite） |
| `http_tasks.json` | 直鏈任務與各分段進度 |
| `bt-session/` | librqbit 的 session 與 DHT 狀態 |
| `logs/app.YYYY-MM-DD.log` | 日誌，每日輪替保留 7 天（`TAURI_DOWNLOADER_LOG` 可調等級） |

## 其他文件

- `CLAUDE.md` — 完整架構表、每個檔案的職責、已知問題與 TODO
- `docs/parallel-rewrite-guide.html` — 「什麼時候該把序列改成並行、怎麼改」的自用指南

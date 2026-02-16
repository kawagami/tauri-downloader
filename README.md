## Tauri + React + Typescript
* 使用 pnpm + vite + react + ts 的 tauri 專案
* 使用 docker 而不安裝 node 的開發環境太麻煩了
* 轉向使用 nvm 安裝 node 後開發 tauri

## windows 開發環境初始要執行的指令
* iwr https://get.pnpm.io/install.ps1 -useb | iex
* pnpm env use --global lts
* pnpm install

## Command
* 開發
    * pnpm tauri dev
* 打包
    * pnpm tauri build

# 流程整理
1. 初始化階段 (Initialization)
    * 啟動獨立線程：使用 thread::spawn 產生一個背景線程，確保監控工作不會卡住 App 的 UI 主畫面。
    * 建立 Async 環境：在線程內建立 Tokio Runtime，讓 Rust 能夠處理非同步的網路請求（reqwest）。
    * 掛載剪貼簿：初始化系統剪貼簿控制器（ClipboardContext）。
2. 監控循環階段 (Monitoring Loop)
    * 定時輪詢：每隔 500ms (0.5秒) 檢查一次剪貼簿內容。
    * 內容比對：判斷目前的內容是否與「上一次紀錄」不同，避免重複觸發。
    * 網址校驗：若內容有變，呼叫 is_valid_wnacg_url 檢查這是否為有效的 wnacg 目標網址。
3. 資料抓取階段 (Data Scraping)
    * 一旦確認網址有效，會啟動一個非同步任務執行以下動作：
    * 發送請求：使用 reqwest 抓取該網址的 HTML 原始碼。
    * 解析 HTML：使用 scraper (CSS 選擇器) 提取特定資訊：
    * 標題：抓取 #bodywrap > h2 的文字。
    * 封面圖：抓取指定的 img 標籤 src 屬性。
    * 下載連結：抓取 #ads > a 的 href，並自動補全網域成完整 URL。
4. 儲存與通知階段 (Persistence & Notification)
    * 寫入資料庫：將抓到的標題、圖片路徑、下載網址等資訊封裝成 ClipboardPayload，存入 SQLite (db::insert_task)。
    * 前端推送：透過 Tauri 的事件機制 (emit) 推送 new-valid-url-payload 事件給前端介面，讓使用者即時看到新抓到的漫畫資訊。

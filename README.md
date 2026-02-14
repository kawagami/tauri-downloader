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

## 目前預計使用 event 改寫目前監控 clipboard 的方式
* [tauir doc](https://v2.tauri.app/develop/calling-frontend/#global-events)

# 相關討論串
* https://gemini.google.com/app/834d91e6ce1b4c4b?hl=zh-TW

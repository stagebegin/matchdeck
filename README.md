# matchdeck · 賽前預測看板

霧青色主題的單檔網頁，由 **Rust（零依賴）** 生成：讀一批賽前預測 JSON，輸出 `index.html` + `data.json`。
沒有框架、沒有 CDN、沒有外部字體，一個檔案就能開。

## 頁面上有什麼

每場比賽一張卡：

- **1X2 概率條**（基線 vs 最終）
- **λ（期望入球）**：調整前／後
- **大小球**（2.5、3.5）與 **BTTS**
- **讓球**：−1 / +1 用負二項 ν=1.4 計
- **Top 比分**
- **信心星**、**首發狀態**（✅ 官方 · ◐ 預測 · ◌ 未公開）、**傷停人數**

下方另有「流程合規狀態」表與「聯賽接駁」清單。

## 資料從哪來

| 用途 | 來源 |
|---|---|
| 賽程、開賽時間、可投注場次 | 香港賽馬會公開賽程（只用於選場，不進模型） |
| 賽果、傷停、陣容、天氣 | FotMob |
| 模型 | 時序 Elo（K=20、主場 +60）→ 時間衰減攻防擬合（半衰期 180 天、先驗 K=2）→ λ → Dixon-Coles 泊松（ρ=−0.08）；讓球另用負二項 |

賽果樣本逐年累積（2023–2026），跨年制聯賽自動處理 `2023/2024` 這種賽季編碼。

## 重新生成

需要 Rust 工具鏈。

```bash
# 輸入：預測檔目錄 + 聯賽對照表
./build.sh ../fb/preds ../fb/registry.json
# 或直接跑
cargo run --release -- --preds ../fb/preds --registry ../fb/registry.json --out index.html
# 保留已開賽的場次
cargo run --release -- --preds ../fb/preds --registry ../fb/registry.json --out index.html --all
```

輸入檔案格式：每場一個 JSON，欄位 `m_id / home / away / comp / kickoff / base{h,d,a,lh,la} / fin{h,d,a,conf} / diag{elo,lambda_adj,injuries,handicap,lineupState,tops}`。

## 設計

霧青色（misty cyan）取自清晨的霧與水色，主色 `#4c9ba1`，底 `#f5fbfb → #d9ecec` 漸層，
卡片是半透明白 + 背景模糊，讓數字浮在霧上而不是压在重色塊裡。

| Token | 值 | 用途 |
|---|---|---|
| `--cyan` | `#4c9ba1` | 主色、聯賽徽章 |
| `--cyan-2/3` | `#7cb8bc` / `#a6cfd1` | 邊框、hover |
| `--home/draw/away` | `#3f8f95` / `#9db6b8` / `#2b616a` | 三路概率條 |
| `--ink` | `#15302f` | 文字 |

## 已知限制

- **Step 2 多智能體共識**與 **Step 5 校準**尚未接入，頁面上也標成「部分／未做」，不假裝有。
- 賠率**不參與**預測，只用於選場與賽後對比。
- 機率是模型估計，不是結果保證。

## 免責

本專案為概率模型與數據整理的展示，**不構成任何投注建議或誘因**。

## License

MIT

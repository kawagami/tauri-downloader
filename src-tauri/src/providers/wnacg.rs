// wnacg provider —— 只負責「爬」：驗證網址、抓元資料、解析出真正的檔案連結。
// 實際下載走共用引擎 crate::dl（與直鏈下載同一套），這裡不再有自己的串流迴圈。

use crate::{
    error::DownloadError,
    providers::{ClipboardPayload, Fetched},
    state::AppState,
};

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use url::Url;

/// 站台主網域（`Site::from_url` 的辨識與這裡的驗證共用）
pub const DOMAIN: &str = "wnacg.com";
/// 規範化用的主機名：裸域與各子網域都收斂到這個，同一部作品才不會因為
/// 有沒有 www（或帶 query）而在 DB 裡變成好幾筆。
const CANONICAL_HOST: &str = "www.wnacg.com";

static RE_VALIDATE: OnceLock<Regex> = OnceLock::new();

fn select_first<'a>(document: &'a Html, selectors: &[&str]) -> Option<ElementRef<'a>> {
    for sel in selectors {
        if let Ok(parsed) = Selector::parse(sel) {
            if let Some(el) = document.select(&parsed).next() {
                return Some(el);
            }
        }
    }
    None
}

/// 驗證 wnacg URL 並回傳規範化的 URL 字串
pub fn validate(content: &str) -> Result<String, String> {
    // 1. 初步解析 URL
    let parsed_url = Url::parse(content).map_err(|_| "無效的 URL 格式".to_string())?;

    // 2. 驗證 Scheme 與 Host (快速過濾)
    if parsed_url.scheme() != "https" {
        return Err("必須使用 https 協定".to_string());
    }

    // host 規則與 Site::from_url 共用：裸域與子網域都收
    let host = parsed_url.host_str().unwrap_or_default();
    if !crate::providers::host_matches(host, DOMAIN) {
        return Err(format!("域名必須為 {}（或其子網域）", DOMAIN));
    }

    // 3. 使用 Regex 驗證 Path 並提取 ID (兼顧檢查與提取)
    let re = RE_VALIDATE.get_or_init(|| Regex::new(r"^/photos-index-aid-(\d+)\.html$").unwrap());

    let id = re
        .captures(parsed_url.path())
        .map(|c| c[1].to_string())
        .ok_or_else(|| "路徑格式錯誤，應為 /photos-index-aid-{ID}.html".to_string())?;

    // 由 ID 重建規範化 URL：丟掉 query/fragment、主機統一，
    // 同一部作品的各種寫法都收斂成同一個 DB 主鍵
    Ok(work_url(id))
}

/// 由 aid 組出規範化的作品頁 URL（validate 與合集章節展開共用）
fn work_url(aid: impl std::fmt::Display) -> String {
    format!("https://{}/photos-index-aid-{}.html", CANONICAL_HOST, aid)
}

/// 從規範化作品頁 URL 取回 aid
fn aid_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let re = RE_VALIDATE.get_or_init(|| Regex::new(r"^/photos-index-aid-(\d+)\.html$").unwrap());
    re.captures(parsed.path()).map(|c| c[1].to_string())
}

/// GET 一頁文字內容；404/410 → NotFound，其他非 2xx → Other
async fn get_text(client: &reqwest::Client, url: &str) -> Result<String, DownloadError> {
    let res = client.get(url).send().await?;

    if matches!(res.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }
    if !res.status().is_success() {
        return Err(DownloadError::Other(format!("網絡請求失敗，狀態碼: {}", res.status())));
    }
    Ok(res.text().await?)
}

/// 站內相對連結補成絕對網址
fn absolutize(raw: &str) -> String {
    if raw.starts_with("http") {
        raw.to_string()
    } else if raw.starts_with("//") {
        format!("https:{}", raw)
    } else {
        format!("https://www.wnacg.com{}", raw)
    }
}

/// 抓下載頁（`download_page_href`），解析出實際 ZIP 檔案連結
pub async fn get_file_url(
    app_handle: &AppHandle,
    url: &str,
) -> Result<String, DownloadError> {
    tracing::debug!("get_file_url: {}", url);

    let state = app_handle.state::<AppState>();
    let html_content = get_text(&state.client, url).await?;
    let document = Html::parse_document(&html_content);

    let raw = select_first(&document, &["#ads > a", "a.ads", "a[href*='down']"])
        .and_then(|el| el.value().attr("href"))
        .ok_or_else(|| DownloadError::Other("wnacg: 無法找到下載連結".to_string()))?;

    Ok(absolutize(raw))
}

/// Range 探測：對實際 ZIP 連結發 `Range: bytes=0-0`，驗證能否真的取到 bytes
/// 並回傳檔案總大小。比 HEAD 可靠（強制走真實下載路徑，有些 CDN 不支援 HEAD）。
/// - 206 Partial：從 `Content-Range: bytes 0-0/{total}` 解析總大小
/// - 200（伺服器忽略 Range）：退回 `Content-Length`
/// - 404/410：回 `NOT_FOUND`，代表連結預檢即失效
async fn probe_file_size(
    client: &reqwest::Client,
    file_url: &str,
) -> Result<i64, DownloadError> {
    let res = client
        .get(file_url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await?;
    let status = res.status();

    if matches!(status, reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE) {
        return Err(DownloadError::NotFound);
    }

    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        if let Some(total) = res
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cr| cr.rsplit('/').next())
            .and_then(|s| s.trim().parse::<i64>().ok())
        {
            return Ok(total);
        }
    }

    if status.is_success() {
        // 伺服器忽略 Range（回 200），退回 Content-Length；拿不到則回 -1（未知）
        return Ok(res.content_length().map(|l| l as i64).unwrap_or(-1));
    }

    Err(DownloadError::Other(format!("探測失敗，狀態碼: {}", status)))
}

/// 作品頁解析結果（解析是同步的，Html 非 Send，不能跨 await）
enum Page {
    Work {
        title: String,
        image: String,
        download_page_href: String,
    },
    /// 合集：本身沒有檔案也沒有下載鈕，只列章節；`sid` 是合集自己的 aid
    Series { title: String, sid: Option<String> },
}

fn parse_page(document: &Html) -> Result<Page, DownloadError> {
    let title = select_first(document, &["#bodywrap > h2", "#bodywrap h2", "h1", "h2"])
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "無法找到標題".to_string());

    // 合集偵測要在找下載鈕之前：合集頁沒有 #ads，照一般作品解析只會得到「找不到下載頁面連結」。
    // 認章節目錄這塊（#sr_pub / .sr_compact 的章節連結），不看標題的「1-5」之類字樣
    if select_first(document, &["#sr_pub", ".sr_compact a[data-chid]"]).is_some() {
        let sid = select_first(document, &["#sr_pub[data-aid]"])
            .and_then(|el| el.value().attr("data-aid"))
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .map(|s| s.to_string());
        return Ok(Page::Series { title, sid });
    }

    let image = select_first(document, &[
        "#bodywrap .pic_box img",
        ".pic_box img",
        ".grid img",
    ])
    .and_then(|el| el.value().attr("src"))
    .map(|s| s.to_string())
    .unwrap_or_else(|| "placeholder.png".to_string());

    let download_page_href = select_first(document, &["#ads > a", "a.ads", "a[href*='down']"])
        .and_then(|el| el.value().attr("href"))
        .map(absolutize)
        .ok_or_else(|| DownloadError::Other("wnacg: 無法找到下載頁面連結".to_string()))?;

    Ok(Page::Work { title, image, download_page_href })
}

/// 合集章節清單 API —— 站方下載頁捲動載入用的同一支，回 JSON：
/// `{code:0, total, page, limit, list:[{id, idx, name, pages, key, dl2}]}`，一頁 `limit` 話（目前 30）
const CHAPTERS_API: &str = "https://www.wnacg.com/?ctl=download&act=chapters";
/// 翻頁上限 —— API 回的 total/limit 不合理時不至於無限翻
const MAX_CHAPTER_PAGES: u64 = 50;

#[derive(serde::Deserialize)]
struct ChapterPage {
    code: i64,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    limit: u64,
    #[serde(default)]
    list: Vec<ChapterItem>,
}

#[derive(serde::Deserialize)]
struct ChapterItem {
    /// 該話本身就是一部普通作品，這是它的 aid
    id: u64,
    /// 第幾話
    #[serde(default)]
    idx: u64,
}

fn parse_chapter_page(body: &str) -> Result<ChapterPage, DownloadError> {
    let page: ChapterPage = serde_json::from_str(body)
        .map_err(|e| DownloadError::Other(format!("wnacg: 章節清單格式錯誤: {}", e)))?;
    if page.code != 0 {
        return Err(DownloadError::Other(format!("wnacg: 章節清單回傳錯誤 code={}", page.code)));
    }
    Ok(page)
}

/// 依話數排序、去重，轉成各話的規範化作品頁 URL。
/// 不信任回傳順序：站方有「倒序」偏好（sr_pref cookie），伺服器可能照偏好排
fn chapter_urls(mut items: Vec<ChapterItem>) -> Vec<String> {
    items.sort_by_key(|c| c.idx);
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|c| seen.insert(c.id))
        .map(|c| work_url(c.id))
        .collect()
}

/// 把章節清單整份翻完。HTML 上的章節目錄同樣有分頁，只解析 HTML 超過一頁就會漏話
async fn fetch_chapter_urls(
    client: &reqwest::Client,
    sid: &str,
) -> Result<Vec<String>, DownloadError> {
    let mut items = Vec::new();
    let mut total = 0;
    for page in 1..=MAX_CHAPTER_PAGES {
        if page > 1 {
            tokio::time::sleep(crate::providers::SERIES_REQUEST_GAP).await;
        }
        let body = get_text(client, &format!("{}&sid={}&page={}", CHAPTERS_API, sid, page)).await?;
        let resp = parse_chapter_page(&body)?;
        total = resp.total;
        let got = resp.list.len();
        items.extend(resp.list);
        if got == 0 || page * resp.limit.max(1) >= resp.total {
            break;
        }
    }
    if (items.len() as u64) < total {
        tracing::warn!("wnacg: 合集 {} 章節清單不完整（{}/{}）", sid, items.len(), total);
    }
    Ok(chapter_urls(items))
}

/// 抓作品頁：一般作品 → 一筆任務（順帶預取 ZIP 連結與大小）；合集 → 各話的作品頁 URL
pub async fn fetch(app_handle: &AppHandle, url: String) -> Result<Fetched, DownloadError> {
    tracing::info!("wnacg fetch: {}", url);

    let state = app_handle.state::<AppState>();
    let client = &state.client;
    let html_content = get_text(client, &url).await?;

    // 用 block 確保 Html（非 Send）在 await 前 drop
    let page = {
        let document = Html::parse_document(&html_content);
        parse_page(&document)?
    };

    match page {
        Page::Series { title, sid } => {
            let sid = sid
                .or_else(|| aid_of(&url))
                .ok_or_else(|| DownloadError::Other("wnacg: 無法取得合集編號".to_string()))?;
            let chapters = fetch_chapter_urls(client, &sid).await?;
            if chapters.is_empty() {
                return Err(DownloadError::Other(format!("wnacg: 合集《{}》沒有任何章節", title)));
            }
            tracing::info!("wnacg: 合集《{}》共 {} 話", title, chapters.len());
            Ok(Fetched::Series { title, chapters })
        }
        Page::Work { title, image, download_page_href } => Ok(Fetched::Work(
            build_work_payload(app_handle, client, url, title, image, download_page_href).await,
        )),
    }
}

/// 一般作品：預取 ZIP 連結 + Range 探測，組成 ClipboardPayload
async fn build_work_payload(
    app_handle: &AppHandle,
    client: &reqwest::Client,
    url: String,
    title: String,
    image: String,
    download_page_href: String,
) -> ClipboardPayload {
    // 順帶抓實際 ZIP URL，快取進 DB 省掉下載時的額外請求；失敗不中斷
    let file_url = get_file_url(app_handle, &download_page_href)
        .await
        .unwrap_or_default();

    // Range 探測：驗證連結真的能下載並取得檔案大小
    let mut file_size: i64 = -1;
    let mut db_status = "idle".to_string();
    if file_url.is_empty() {
        tracing::warn!("wnacg: 無法預取 file_url，下載時將重新抓取");
    } else {
        match probe_file_size(client, &file_url).await {
            Ok(size) => file_size = size,
            Err(DownloadError::NotFound) => {
                // 預檢就確定 ZIP 連結已失效，直接標 not_found
                tracing::warn!("wnacg: ZIP 連結預檢 404/410: {}", file_url);
                db_status = "not_found".to_string();
            }
            Err(e) => {
                // 暫時性失敗，大小未知，仍以 idle 加入、下載時再試
                tracing::warn!("wnacg: 大小探測失敗（{}），標為未知", e);
            }
        }
    }

    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    ClipboardPayload {
        url,
        title,
        image,
        download_page_href,
        file_url,
        file_size,
        created_at,
        db_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str = "https://www.wnacg.com/photos-index-aid-123.html";

    /// 各種寫法都要收斂成同一個規範化 URL —— DB 主鍵是 url，
    /// 不收斂的話同一部作品會變成好幾筆
    #[test]
    fn canonicalizes_host_and_query() {
        for input in [
            CANONICAL,
            "https://wnacg.com/photos-index-aid-123.html",
            "https://m.wnacg.com/photos-index-aid-123.html",
            "https://www.wnacg.com/photos-index-aid-123.html?from=list#top",
        ] {
            assert_eq!(validate(input).as_deref(), Ok(CANONICAL), "input={input}");
        }
    }

    /// from_url 認得的 host，validate 就不能拒絕（以前無 www 會靜默失敗）
    #[test]
    fn accepts_every_host_from_url_accepts() {
        let bare = "https://wnacg.com/photos-index-aid-123.html";
        assert!(crate::providers::Site::from_url(bare).is_ok());
        assert!(validate(bare).is_ok());
    }

    /// 合集頁（實際頁面節錄）：沒有 #ads，要認成 Series 而不是「找不到下載頁面連結」
    #[test]
    fn detects_series_page() {
        let html = r#"<div id="bodywrap" class="userwrap"><h2>[abgrund] おにいちゃんコントローラー 1-5</h2>
            <label>章節：5 話</label></div>
            <div id="bodywrap" class="cc">
              <div class="cc filter sr_filter" id="sr_pub" data-order="asc" data-mode="list" data-aid="390942"></div>
              <div class="sr_compact">
                <a class="tagshow" data-chid="34913" href="/photos-slide-aid-34913-sid-390942.html">第1話</a>
              </div>
            </div>"#;
        match parse_page(&Html::parse_document(html)).ok() {
            Some(Page::Series { title, sid }) => {
                assert_eq!(title, "[abgrund] おにいちゃんコントローラー 1-5");
                assert_eq!(sid.as_deref(), Some("390942"));
            }
            _ => panic!("應該認成合集"),
        }
    }

    #[test]
    fn parses_work_page() {
        let html = r#"<div id="bodywrap" class="userwrap"><h2>作品</h2>
            <div id="ads" class="ads"><a class="btn" href="/download-index-aid-390908.html">下載漫畫</a></div></div>"#;
        match parse_page(&Html::parse_document(html)).ok() {
            Some(Page::Work { title, download_page_href, .. }) => {
                assert_eq!(title, "作品");
                assert_eq!(download_page_href, "https://www.wnacg.com/download-index-aid-390908.html");
            }
            _ => panic!("應該認成一般作品"),
        }
    }

    /// 章節 API 回應（實際格式）：依 idx 排序、去重，轉成各話規範化作品頁 URL
    #[test]
    fn chapter_list_sorted_and_deduped() {
        let body = r#"{"code":0,"total":3,"page":1,"limit":30,"list":[
            {"id":73321,"idx":3,"name":"c","pages":19,"key":"k","dl2":"//x"},
            {"id":34913,"idx":1,"name":"a","pages":20,"key":"k","dl2":"//x"},
            {"id":30698,"idx":2,"name":"b","pages":53,"key":"k","dl2":"//x"},
            {"id":34913,"idx":1,"name":"a","pages":20,"key":"k","dl2":"//x"}]}"#;
        let Ok(page) = parse_chapter_page(body) else { panic!("應可解析") };
        assert_eq!(page.total, 3);
        assert_eq!(
            chapter_urls(page.list),
            vec![work_url(34913), work_url(30698), work_url(73321)]
        );
        assert!(validate(&work_url(34913)).is_ok());
    }

    #[test]
    fn chapter_list_error_code_is_err() {
        assert!(parse_chapter_page(r#"{"code":1,"msg":"bad"}"#).is_err());
        assert!(parse_chapter_page("<html>503</html>").is_err());
    }

    #[test]
    fn rejects_wrong_scheme_host_and_path() {
        assert!(validate("http://www.wnacg.com/photos-index-aid-123.html").is_err());
        assert!(validate("https://evil-wnacg.com/photos-index-aid-123.html").is_err());
        assert!(validate("https://www.wnacg.com/photos-slist-aid-123.html").is_err());
    }
}

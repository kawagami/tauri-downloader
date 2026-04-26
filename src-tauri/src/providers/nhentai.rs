use regex::Regex;
use std::sync::OnceLock;
use url::Url;

static RE_VALIDATE: OnceLock<Regex> = OnceLock::new();

/// 驗證 nhentai URL 並回傳規範化的 URL 字串
pub fn validate(content: &str) -> Result<String, String> {
    // 1. 初步解析 URL
    let parsed_url = Url::parse(content).map_err(|_| "無效的 URL 格式".to_string())?;

    // 2. 驗證 Scheme 與 Host
    // nhentai 強制使用 https，且主網域為 nhentai.net
    if parsed_url.scheme() != "https" {
        return Err("必須使用 https 協定".to_string());
    }

    if parsed_url.host_str() != Some("nhentai.net") {
        return Err("域名必須為 nhentai.net".to_string());
    }

    // 3. 使用 Regex 驗證 Path 並提取 ID
    // 格式通常為 /g/123456/ 或 /g/123456
    let re = RE_VALIDATE.get_or_init(|| Regex::new(r"^/g/(\d+)/?$").unwrap());

    if !re.is_match(parsed_url.path()) {
        return Err("路徑格式錯誤，應為 /g/{ID}/".to_string());
    }

    // 4. 規範化：確保回傳的格式統一（例如統一加上結尾斜線，或去掉 query parameters）
    let caps = re.captures(parsed_url.path()).unwrap();
    let id = &caps[1];

    // 建構標準化的 URL 避免帶有額外的參數或不一致的斜線
    Ok(format!("https://nhentai.net/g/{}/", id))
}

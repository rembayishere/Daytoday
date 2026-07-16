use reqwest::Method;
use crate::models::{WebdavConfig, SettingsData};
use crate::crypto;
use std::sync::OnceLock;

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("无法初始化 HTTP 客户端 (TLS 后端加载失败)")
    })
}

pub(crate) async fn send_request(
    method: &[u8],
    url: &str,
    cfg: &WebdavConfig,
    body: Option<Vec<u8>>,
    timeout_secs: u64,
) -> Result<reqwest::Response, String> {
    let client = http_client();
    let mut req = client
        .request(Method::from_bytes(method).unwrap(), url)
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .timeout(std::time::Duration::from_secs(timeout_secs));
    if let Some(b) = body {
        req = req.header("Content-Type", "application/octet-stream").body(b);
    }
    let method_str = String::from_utf8_lossy(method);
    println!("[webdav] {} {}", method_str, url);
    let resp = req.send().await.map_err(|e| format!("请求失败: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        println!("[webdav] {} -> HTTP {} (user='{}', url='{}')", method_str, status, cfg.user, url);
    }
    Ok(resp)
}

async fn body_text(resp: reqwest::Response) -> String {
    resp.text().await.unwrap_or_default()
}

pub async fn test_connection(cfg: &WebdavConfig) -> Result<String, String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let root = base_url.clone();
    let resp = send_request(b"PROPFIND", &root, cfg, None, 10).await?;
    let status = resp.status();

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err("401 未授权：账号或密码错误。注意 WebDAV 密码通常是服务商提供的「专用应用密码/授权码」，不是登录密码".into());
    }
    if !(status.is_success() || status.as_u16() == 207) {
        let body = body_text(resp).await;
        return Err(format!("服务器返回 {}: {}", status, body));
    }

    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/", base_url, path);
    let mk = send_request(b"MKCOL", &target, cfg, None, 10).await?;
    let mk_status = mk.status().as_u16();
    if mk_status == 201 || mk_status == 405 || mk_status == 401 {
        if mk_status == 401 {
            return Err("根目录可访问，但同步子路径无权限（401）。请确认同步路径正确，且账号对该目录有写入权限".into());
        }
        Ok("WebDAV 连接成功，同步目录可用".into())
    } else {
        let body = body_text(mk).await;
        Err(format!("连接成功，但无法创建同步目录 (HTTP {})：{}", mk_status, body))
    }
}

pub async fn ensure_dir(cfg: &WebdavConfig) -> Result<(), String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/", base_url, path);

    let resp = send_request(b"MKCOL", &target, cfg, None, 10).await?;
    let status = resp.status().as_u16();

    match status {
        201 | 405 => Ok(()),
        _ => {
            let body = body_text(resp).await;
            Err(format!("创建目录失败 (HTTP {}) 目标: {} | {}", status, target, body))
        }
    }
}

pub async fn sync_upload(cfg: &WebdavConfig, data: &[u8]) -> Result<String, String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/daytoday-backup.json", base_url, path);

    ensure_dir(cfg).await?;

    let payload = if cfg.encrypt && !cfg.enc_pass.is_empty() {
        let enc_pass = cfg.enc_pass.clone();
        let data_owned = data.to_vec();
        tokio::task::spawn_blocking(move || {
            crypto::encrypt(&data_owned, &enc_pass).map(|s| s.into_bytes())
        }).await.map_err(|e| format!("后台任务失败: {}", e))?
          .map_err(|e| format!("加密失败: {}", e))?
    } else {
        data.to_vec()
    };

    let resp = send_request(b"PUT", &target, cfg, Some(payload), 120).await?;
    let status = resp.status();

    if status.is_success() {
        let enc = if cfg.encrypt && !cfg.enc_pass.is_empty() { "加密" } else { "未加密" };
        Ok(format!("同步完成 · {}上传至 /{}", enc, path))
    } else {
        let body = body_text(resp).await;
        Err(format!("上传失败 (HTTP {}) 目标: {} | {}", status, target, body))
    }
}

pub async fn download_raw(cfg: &WebdavConfig) -> Result<Vec<u8>, String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/daytoday-backup.json", base_url, path);
    let resp = send_request(b"GET", &target, cfg, None, 30).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = body_text(resp).await;
        return Err(format!("下载失败 (HTTP {}): {}", status, body));
    }
    resp.bytes().await.map_err(|e| e.to_string()).map(|b| b.to_vec())
}

pub async fn sync_download(cfg: &WebdavConfig) -> Result<Vec<u8>, String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/daytoday-backup.json", base_url, path);

    let resp = send_request(b"GET", &target, cfg, None, 30).await?;
    let status = resp.status();

    if !status.is_success() {
        let body = body_text(resp).await;
        return Err(format!("下载失败 (HTTP {}): {}", status, body));
    }

    let body = resp.bytes().await.map_err(|e| e.to_string())?;

    if cfg.encrypt && !cfg.enc_pass.is_empty() {
        let text = String::from_utf8(body.to_vec()).map_err(|e| format!("解码失败: {}", e))?;
        crypto::decrypt(&text, &cfg.enc_pass)
    } else {
        Ok(body.to_vec())
    }
}

fn settings_pass(cfg: &WebdavConfig) -> Option<String> {
    if !cfg.settings_pass.is_empty() {
        Some(cfg.settings_pass.clone())
    } else if !cfg.enc_pass.is_empty() {
        Some(cfg.enc_pass.clone())
    } else {
        None
    }
}

pub async fn upload_settings(cfg: &WebdavConfig, settings: &SettingsData) -> Result<String, String> {
    let pass = settings_pass(cfg).ok_or("未设置加密密码，设置无法加密存储，请填写「设置加密密码」或「加密密码」")?;
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/daytoday-settings.json", base_url, path);

    ensure_dir(cfg).await?;

    let json = serde_json::to_vec(settings).map_err(|e| e.to_string())?;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&json).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;

    let payload = tokio::task::spawn_blocking(move || {
        crypto::encrypt(&compressed, &pass).map(|s| s.into_bytes())
    }).await.map_err(|e| format!("后台任务失败: {}", e))?
      .map_err(|e| format!("设置加密失败: {}", e))?;

    let resp = send_request(b"PUT", &target, cfg, Some(payload), 120).await?;
    let status = resp.status();
    if status.is_success() {
        Ok("设置已加密上传至云端".into())
    } else {
        let body = body_text(resp).await;
        Err(format!("设置上传失败 (HTTP {}): {}", status, body))
    }
}

pub async fn download_settings(cfg: &WebdavConfig) -> Result<SettingsData, String> {
    let pass = settings_pass(cfg).ok_or("未设置加密密码，无法解密云端设置")?;
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/daytoday-settings.json", base_url, path);

    let resp = send_request(b"GET", &target, cfg, None, 30).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = body_text(resp).await;
        return Err(format!("设置下载失败 (HTTP {}): {}", status, body));
    }

    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    let b64_str = String::from_utf8(body.to_vec()).map_err(|e| format!("设置解码失败: {}", e))?;
    let decrypted = crypto::decrypt(&b64_str, &pass)?;

    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut decoder = GzDecoder::new(&decrypted[..]);
    let mut s = String::new();
    decoder.read_to_string(&mut s).map_err(|e| e.to_string())?;

    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub async fn upload_file(cfg: &WebdavConfig, local_path: &str, filename: &str) -> Result<String, String> {
    if !cfg.encrypt || cfg.enc_pass.is_empty() {
        return Err("附件上传需要启用加密并设置加密密码".into());
    }
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/{}", base_url, path, filename);

    let file_bytes = std::fs::read(local_path).map_err(|e| format!("读取附件失败: {}", e))?;
    let enc_pass = cfg.enc_pass.clone();

    let payload = tokio::task::spawn_blocking(move || {
        crypto::encrypt(&file_bytes, &enc_pass).map(|s| s.into_bytes())
    }).await.map_err(|e| format!("后台任务失败: {}", e))?
      .map_err(|e| format!("附件加密失败: {}", e))?;

    let resp = send_request(b"PUT", &target, cfg, Some(payload), 120).await?;
    let status = resp.status();
    if status.is_success() {
        Ok(format!("附件 {} 已上传", filename))
    } else {
        let body = body_text(resp).await;
        Err(format!("附件上传失败 (HTTP {}): {}", status, body))
    }
}

pub async fn download_file(cfg: &WebdavConfig, filename: &str, local_dir: &str) -> Result<String, String> {
    if !cfg.encrypt || cfg.enc_pass.is_empty() {
        return Err("附件下载需要启用加密并设置加密密码".into());
    }
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/{}", base_url, path, filename);

    let resp = send_request(b"GET", &target, cfg, None, 60).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = body_text(resp).await;
        return Err(format!("附件下载失败 (HTTP {}): {}", status, body));
    }

    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    let b64_str = String::from_utf8(body.to_vec()).map_err(|e| e.to_string())?;
    let enc_pass = cfg.enc_pass.clone();

    let decrypted = tokio::task::spawn_blocking(move || {
        crypto::decrypt(&b64_str, &enc_pass)
    }).await.map_err(|e| format!("后台任务失败: {}", e))??;

    let local_file = std::path::PathBuf::from(local_dir).join(filename);
    std::fs::create_dir_all(local_file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&local_file, &decrypted).map_err(|e| e.to_string())?;
    Ok(format!("附件 {} 已下载", filename))
}

/// 上传后校验：用 PROPFIND 确认服务端确实存在该文件
pub async fn remote_file_exists(cfg: &WebdavConfig, remote_path: &str) -> bool {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/{}", base_url, path, remote_path);
    match send_request(b"PROPFIND", &target, cfg, None, 10).await {
        Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 207,
        Err(_) => false,
    }
}

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:href/>
    <d:getcontentlength/>
    <d:getcontenttype/>
  </d:prop>
</d:propfind>"#;

/// 对某个目录发 PROPFIND（Depth:1），返回响应体文本
pub async fn propfind_dir(cfg: &WebdavConfig, target: &str) -> Result<String, String> {
    let client = http_client();
    let req = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), target)
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .header("Depth", "1")
        .timeout(std::time::Duration::from_secs(15))
        .body(PROPFIND_BODY);
    let resp = req.send().await.map_err(|e| format!("请求失败: {}", e))?;
    let status = resp.status();
    if !(status.is_success() || status.as_u16() == 207) {
        return Err(format!("PROPFIND 失败 (HTTP {})", status));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// 解析 WebDAV multistatus XML，提取子文件（排除目录自身）的名称与大小
pub fn parse_propfind(body: &str, local_names: &[String]) -> Vec<crate::models::RemoteAttachment> {
    let mut results = Vec::new();
    // 按 <response> 分块
    let blocks: Vec<&str> = body.split("<response").skip(1).collect();
    for block in blocks {
        // 取该 response 内的 href 作为文件名基准
        let href = match extract_tag(block, "href").first().cloned() {
            Some(h) => h,
            None => continue,
        };
        let decoded = decode_href(&href);
        let name = match decoded.rsplit('/').nth(0) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        // 跳过目录自身（以 / 结尾或 . 目录）
        if decoded.ends_with('/') || name == "." || name == ".." {
            continue;
        }
        // 跳过非附件目录下的其它（理论上已在 attachments/ 下）
        let size = extract_tag(block, "getcontentlength")
            .first()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let exists_local = local_names.iter().any(|n| n == &name);
        results.push(crate::models::RemoteAttachment {
            filename: name,
            size,
            exists_local,
        });
    }
    results
}

fn decode_href(href: &str) -> String {
    // 处理 %XX URL 编码（如中文/空格）
    let bytes = href.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn extract_tag<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut s = xml;
    loop {
        // 找一个开标签： <tag ...> 或 <ns:tag ...>
        let open = match s.find('<') {
            Some(i) => i,
            None => break,
        };
        let rest = &s[open + 1..];
        // 标签名（忽略命名空间前缀 ns:）
        let name = match rest.find('>') {
            Some(end) => &rest[..end],
            None => break,
        };
        let name = name.split_whitespace().next().unwrap_or("");
        let name = name.split(':').last().unwrap_or(name);
        if name != tag {
            s = &rest[rest.find('>').unwrap_or(0) + 1..];
            continue;
        }
        let tag_end = rest.find('>').unwrap_or(rest.len() - 1);
        // 闭合标签： </tag> 或 </ns:tag>
        let close = format!("</{}>", tag);
        let close_ns = format!("</d:{}>", tag);
        let end = match rest[tag_end..].find(&close).or_else(|| rest[tag_end..].find(&close_ns)) {
            Some(e) => tag_end + e,
            None => { s = &rest[tag_end + 1..]; continue; }
        };
        out.push(rest[tag_end + 1..end].trim());
        s = &rest[end + close.len()..];
    }
    out
}

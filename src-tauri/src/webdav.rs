use reqwest::Method;
use crate::models::{WebdavConfig, SettingsData};
use crate::crypto;
use std::sync::OnceLock;

// 取加密算法，缺省回退到 aes256-gcm（兼容旧配置/旧数据）
fn enc_alg(cfg: &WebdavConfig) -> &str {
    if cfg.enc_algorithm.is_empty() { crypto::default_algorithm() } else { &cfg.enc_algorithm }
}

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
        let alg = enc_alg(cfg).to_string();
        let data_owned = data.to_vec();
        tokio::task::spawn_blocking(move || {
            crypto::encrypt(&data_owned, &enc_pass, &alg).map(|s| s.into_bytes())
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
        crypto::decrypt(&text, &cfg.enc_pass, enc_alg(cfg))
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

    let alg = enc_alg(cfg).to_string();
    let payload = tokio::task::spawn_blocking(move || {
        crypto::encrypt(&compressed, &pass, &alg).map(|s| s.into_bytes())
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
    let decrypted = crypto::decrypt(&b64_str, &pass, enc_alg(cfg))?;

    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut decoder = GzDecoder::new(&decrypted[..]);
    let mut s = String::new();
    decoder.read_to_string(&mut s).map_err(|e| e.to_string())?;

    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub async fn upload_file(cfg: &WebdavConfig, local_path: &str, filename: &str) -> Result<String, String> {
    let encrypted = cfg.encrypt && !cfg.enc_pass.is_empty();
    if !encrypted && !cfg.allow_unencrypted_attachment {
        return Err("附件上传需要启用加密并设置加密密码（或在设置中开启「允许明文上传附件」）".into());
    }
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/{}", base_url, path, filename);

    let file_bytes = std::fs::read(local_path).map_err(|e| format!("读取附件失败: {}", e))?;
    let payload: Vec<u8> = if encrypted {
        let enc_pass = cfg.enc_pass.clone();
        let alg = enc_alg(cfg).to_string();
        tokio::task::spawn_blocking(move || {
            crypto::encrypt(&file_bytes, &enc_pass, &alg).map(|s| s.into_bytes())
        }).await.map_err(|e| format!("后台任务失败: {}", e))?
          .map_err(|e| format!("附件加密失败: {}", e))?
    } else {
        file_bytes
    };

    let resp = send_request(b"PUT", &target, cfg, Some(payload), 120).await?;
    let status = resp.status();
    if status.is_success() {
        Ok(format!("附件 {} 已上传{}", filename, if encrypted { "" } else { "（明文）" }))
    } else {
        let body = body_text(resp).await;
        Err(format!("附件上传失败 (HTTP {}): {}", status, body))
    }
}

/// 下载附件原始字节（不解密），用于加密校验
pub async fn download_file_bytes(cfg: &WebdavConfig, filename: &str) -> Result<Vec<u8>, String> {
    let encrypted = cfg.encrypt && !cfg.enc_pass.is_empty();
    if !encrypted && !cfg.allow_unencrypted_attachment {
        return Err("附件下载需要启用加密并设置加密密码（或在设置中开启「允许明文下载附件」）".into());
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
    resp.bytes().await.map_err(|e| e.to_string()).map(|b| b.to_vec())
}

/// 明文下载附件文件到本地（未启用加密时使用）
pub async fn download_file_raw(cfg: &WebdavConfig, filename: &str, local_dir: &str) -> Result<String, String> {
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
    let local_file = std::path::PathBuf::from(local_dir).join(filename);
    std::fs::create_dir_all(local_file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&local_file, &body).map_err(|e| e.to_string())?;
    Ok(format!("附件 {} 已下载（明文）", filename))
}

pub async fn download_file(cfg: &WebdavConfig, filename: &str, local_dir: &str) -> Result<String, String> {
    let encrypted = cfg.encrypt && !cfg.enc_pass.is_empty();
    if !encrypted {
        if cfg.allow_unencrypted_attachment {
            return download_file_raw(cfg, filename, local_dir).await;
        }
        return Err("附件下载需要启用加密并设置加密密码（或在设置中开启「允许明文下载附件」）".into());
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
    let alg = enc_alg(cfg).to_string();

    let decrypted = tokio::task::spawn_blocking(move || {
        crypto::decrypt(&b64_str, &enc_pass, &alg)
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

/// 确保云端 attachments 目录存在（用于列出/校验前，避免因目录不存在而 404）
pub async fn ensure_attachment_dir(cfg: &WebdavConfig) -> Result<(), String> {
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/", base_url, path);
    let resp = send_request(b"MKCOL", &target, cfg, None, 10).await?;
    let status = resp.status().as_u16();
    match status {
        201 | 405 => Ok(()),
        _ => {
            let body = body_text(resp).await;
            Err(format!("创建云端附件目录失败 (HTTP {}) 目标: {} | {}", status, target, body))
        }
    }
}

/// 对某个目录发 PROPFIND（Depth:1），返回响应体文本
pub async fn propfind_dir(cfg: &WebdavConfig, target: &str) -> Result<String, String> {
    let (_, body) = propfind_with_status(cfg, target).await?;
    Ok(body)
}

/// 与 propfind_dir 相同，但额外返回 HTTP 状态码，供调试命令使用
pub async fn propfind_with_status(cfg: &WebdavConfig, target: &str) -> Result<(u16, String), String> {
    let client = http_client();
    let req = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), target)
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .header("Depth", "1")
        .timeout(std::time::Duration::from_secs(15))
        .body(PROPFIND_BODY);
    let resp = req.send().await.map_err(|e| format!("请求失败: {}", e))?;
    let status = resp.status().as_u16();
    if !(resp.status().is_success() || status == 207) {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("PROPFIND 失败 (HTTP {}) 响应: {}", status, body));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    Ok((status, body))
}

/// 查找下一个 <response 起始位置（支持命名空间前缀，如 <D:response / <d:response / <response）
fn find_response_start(body: &str, from: usize) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut i = from;
    while i < body.len() {
        if bytes[i] == b'<' {
            // 取 < 之后到空格/'>'/'>' 的标签名（可能含命名空间前缀 ns:）
            let rest = &body[i + 1..];
            let end = rest.find(|c| c == ' ' || c == '>' || c == '/').unwrap_or(rest.len());
            let tag = &rest[..end];
            // 去掉命名空间前缀 ns: 后应为 response
            let local = match tag.rfind(':') {
                Some(p) => &tag[p + 1..],
                None => tag,
            };
            if local == "response" {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// 按 <response>（含命名空间前缀如 <d:response>）切分为独立 block
fn split_responses(body: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let start = match find_response_start(body, i) {
            Some(s) => s,
            None => break,
        };
        // 找到下一个 <response 起始作为块结束
        let next = match find_response_start(body, start + 1) {
            Some(s) => s,
            None => body.len(),
        };
        blocks.push(&body[start..next]);
        i = next;
    }
    blocks
}

/// 查找下一个 <tag（命名空间无关，如 <D:tag / <tag）起始 '<' 位置，从 from 开始
fn find_tag_start(body: &str, from: usize, tag: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut i = from;
    while i < body.len() {
        if bytes[i] == b'<' {
            let rest = &body[i + 1..];
            let end = rest.find(|c| c == ' ' || c == '>' || c == '/').unwrap_or(rest.len());
            let name = &rest[..end];
            let local = match name.rfind(':') {
                Some(p) => &name[p + 1..],
                None => name,
            };
            if local == tag {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// 取 <response> 直接子级的 <href>（即该条目的路径），跳过 <propstat> 内部的空 href
fn extract_response_href(block: &str) -> Option<String> {
    // 找到第一个 propstat 之前的范围作为「条目头」，href 通常在这里
    let head = match find_tag_start(block, 0, "propstat") {
        Some(p) => &block[..p],
        None => block,
    };
    extract_tag(head, "href").into_iter().find(|h| !h.is_empty()).map(|s| s.to_string())
}

/// 从 <response> 中提取 200 OK 的 <propstat> 内的 getcontentlength
fn extract_content_length(block: &str) -> u64 {
    // 按 <propstat>（含命名空间前缀）分块，找到 status 含 "200" 的那块取 getcontentlength
    let mut i = 0;
    while i < block.len() {
        let start = match find_tag_start(block, i, "propstat") {
            Some(p) => p,
            None => break,
        };
        let end = match find_tag_start(block, start + 1, "propstat")
            .or_else(|| block[start..].find("</response>").map(|e| start + e))
        {
            Some(e) => e,
            None => block.len(),
        };
        let chunk = &block[start..end];
        if extract_tag(chunk, "status").iter().any(|s| s.contains("200")) {
            if let Some(s) = extract_tag(chunk, "getcontentlength").first() {
                if let Ok(v) = s.trim().parse::<u64>() {
                    return v;
                }
            }
        }
        i = end;
    }
    0
}

/// 解析 WebDAV multistatus XML，提取子文件（排除目录自身）的名称与大小
pub fn parse_propfind(body: &str, local_names: &[String]) -> Vec<crate::models::RemoteAttachment> {
    let mut results = Vec::new();
    // 按 <response>（含命名空间前缀）分块
    let blocks = split_responses(body);
    for block in blocks {
        // 取该 response 直接子级 href（propstat 内的空 href 会干扰，需排除）
        let href = match extract_response_href(block) {
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
        let size = extract_content_length(block);
        let exists_local = local_names.iter().any(|n| n == &name);
        results.push(crate::models::RemoteAttachment {
            filename: name,
            size,
            exists_local,
        });
    }
    results
}

pub fn decode_href(href: &str) -> String {
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
    while let Some(open) = s.find('<') {
        let rest = &s[open + 1..];
        // 读取本地标签名（忽略命名空间前缀 ns:）
        let tag_end_open = match rest.find('>') {
            Some(e) => e,
            None => break,
        };
        let raw_name = &rest[..tag_end_open];
        let local = raw_name.split_whitespace().next().unwrap_or("").split(':').last().unwrap_or("");
        if local != tag {
            s = &rest[tag_end_open + 1..];
            continue;
        }
        // 内容从 '>' 之后开始
        let content_start = tag_end_open + 1;
        let after = &rest[content_start..];
        // 查找闭合标签：优先精确匹配 </tag>，否则命名空间无关匹配 </ns:tag>
        let close_pos = {
            let exact = format!("</{}>", tag);
            if let Some(cp) = after.find(&exact) {
                Some(cp)
            } else {
                let mut p = after.find("</");
                let mut found = None;
                while let Some(pos) = p {
                    let mut k = pos + 2;
                    // 仅跳过命名空间前缀（字母数字 + 一个冒号），冒号后即为标签名
                    while k < after.len() && after.as_bytes()[k].is_ascii_alphanumeric() {
                        k += 1;
                    }
                    if k < after.len() && after.as_bytes()[k] == b':' {
                        k += 1;
                    }
                    if after[k..].starts_with(tag)
                        && after.as_bytes().get(k + tag.len()) == Some(&b'>')
                    {
                        found = Some(pos);
                        break;
                    }
                    p = after[pos + 2..].find("</").map(|x| pos + 2 + x);
                }
                found
            }
        };
        match close_pos {
            Some(cp) => {
                let content = after[..cp].trim();
                out.push(content);
                s = &after[cp + 1..];
                // 跳过闭合标签剩余部分
                if let Some(gt) = s.find('>') {
                    s = &s[gt + 1..];
                }
            }
            None => {
                s = &rest[tag_end_open + 1..];
                continue;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?><D:multistatus xmlns:D="DAV:"><D:response><D:href>/webdav/backup/attachments/</D:href><D:propstat><D:prop><D:href></D:href><D:getcontentlength></D:getcontentlength><D:getcontenttype></D:getcontenttype></D:prop><D:status>HTTP/1.1 404 Not Found</D:status></D:propstat></D:response><D:response><D:href>/webdav/backup/attachments/touch%E7%AD%BE%E5%AD%97%E7%AC%94%E6%98%A0%E5%B0%84%E9%97%AE%E9%A2%98%E8%A7%A3%E5%86%B3%281%29.zip</D:href><D:propstat><D:prop><D:getcontentlength>86384</D:getcontentlength><D:getcontenttype></D:getcontenttype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat><D:propstat><D:prop><D:href></D:href></D:prop><D:status>HTTP/1.1 404 Not Found</D:status></D:propstat></D:response></D:multistatus>"#;

    #[test]
    fn parses_123pan_attachments() {
        let blocks = split_responses(SAMPLE);
        for b in &blocks {
            eprintln!("CLEN for block: {:?}", extract_content_length(b));
        }
        let res = parse_propfind(SAMPLE, &[]);
        assert_eq!(res.len(), 1, "should find exactly 1 file (dir self skipped)");
        assert!(res[0].filename.starts_with("touch"), "filename={}", res[0].filename);
        assert!(res[0].filename.ends_with(".zip"), "filename={}", res[0].filename);
        assert_eq!(res[0].size, 86384);
    }
}

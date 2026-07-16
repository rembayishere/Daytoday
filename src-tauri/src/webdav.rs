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

use tauri::State;
use tauri::Emitter;
use tauri::Manager;
use crate::models::*;
use crate::storage;
use crate::webdav;
use crate::crypto;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct AppState(pub Mutex<AppData>);

fn get_attachment_dir(app: &tauri::AppHandle, data: &AppData) -> std::path::PathBuf {
    if data.attachment_dir.is_empty() {
        app.path().app_data_dir().unwrap().join("attachments")
    } else {
        std::path::PathBuf::from(&data.attachment_dir)
    }
}

fn today() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d").to_string()
}

fn now_iso() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn gen_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed) % 1000;
    ts * 1000 + counter
}

const MAX_VERSIONS: usize = 50;

fn push_version(q: &mut Question) {
    if !q.versions.is_empty() {
        let prev = q.versions.last().unwrap();
        let mut changes = Vec::new();
        if prev.question != q.question { changes.push("标题更新".into()); }
        if prev.status != q.status { changes.push(format!("状态从「{}」→「{}」", prev.status.label(), q.status.label())); }
        if prev.note != q.note { changes.push("备注更新".into()); }
        if prev.tags != q.tags { changes.push("标签变更".into()); }
        if prev.desc != q.desc { changes.push("描述更新".into()); }
        let change_desc = if changes.is_empty() { "更新了信息".into() } else { changes.join("；") };
        q.versions.push(Version {
            id: gen_id(),
            q_id: q.id,
            timestamp: now_iso(),
            question: q.question.clone(),
            status: q.status.clone(),
            desc: q.desc.clone(),
            note: q.note.clone(),
            tags: q.tags.clone(),
            change_desc,
        });
        while q.versions.len() > MAX_VERSIONS { q.versions.remove(0); }
    } else {
        q.versions.push(Version {
            id: gen_id(),
            q_id: q.id,
            timestamp: now_iso(),
            question: q.question.clone(),
            status: q.status.clone(),
            desc: q.desc.clone(),
            note: q.note.clone(),
            tags: q.tags.clone(),
            change_desc: "创建问题".into(),
        });
    }
}

fn merge_into_local(local: &mut AppData, remote: &AppData) {
    merge_by_id(&mut local.notes, &remote.notes);
    merge_summaries(&mut local.summaries, &remote.summaries);
    merge_by_id(&mut local.clips, &remote.clips);
    merge_by_id(&mut local.questions, &remote.questions);
    merge_by_id(&mut local.flashcards, &remote.flashcards);
    merge_by_id(&mut local.attachments, &remote.attachments);
    merge_by_id(&mut local.tasks.todo, &remote.tasks.todo);
    merge_by_id(&mut local.tasks.doing, &remote.tasks.doing);
    merge_by_id(&mut local.tasks.done, &remote.tasks.done);
}

// === Data Loading ===

#[tauri::command]
pub fn get_all_data(state: State<'_, AppState>) -> Result<AppData, String> {
    let data = state.0.lock().unwrap();
    Ok(data.clone())
}

// === Notes CRUD ===

#[tauri::command]
pub fn add_note(app: tauri::AppHandle, state: State<'_, AppState>, text: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let note = Note {
        id: gen_id(),
        text,
        tags: vec![],
        time: now_iso(),
        date: today(),
    };
    data.notes.push(note.clone());
    storage::save(&app, &data)?;
    app.emit("note-added", note.clone()).ok();
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_note(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    data.notes.retain(|n| n.id != id);
    storage::save(&app, &data)?;
    app.emit("note-deleted", id).ok();
    Ok(data.clone())
}

#[tauri::command]
pub fn update_note(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, text: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(note) = data.notes.iter_mut().find(|n| n.id == id) {
        note.text = text;
    }
    storage::save(&app, &data)?;
    app.emit("note-updated", id).ok();
    Ok(data.clone())
}

// === Clips CRUD ===

#[tauri::command]
pub fn add_clip(app: tauri::AppHandle, state: State<'_, AppState>, title: String, url: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let clip = Clip {
        id: gen_id(),
        title,
        url,
    };
    data.clips.push(clip);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_clip(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    data.clips.retain(|c| c.id != id);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn update_clip(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, title: String, url: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(clip) = data.clips.iter_mut().find(|c| c.id == id) {
        clip.title = title;
        clip.url = url;
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

// === Questions CRUD ===

#[tauri::command]
pub fn add_question(app: tauri::AppHandle, state: State<'_, AppState>, question: String, desc: String, tags: Vec<String>) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let q = Question {
        id: gen_id(),
        question,
        desc,
        note: String::new(),
        status: QuestionStatus::Open,
        tags,
        created: now_iso(),
        date: today(),
        versions: vec![],
    };
    data.questions.push(q);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn update_question(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, question: String, desc: String, tags: Vec<String>) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(q) = data.questions.iter_mut().find(|q| q.id == id) {
        q.question = question;
        q.desc = desc;
        q.tags = tags;
        push_version(q);
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_question(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    data.questions.retain(|q| q.id != id);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn cycle_question(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(q) = data.questions.iter_mut().find(|q| q.id == id) {
        q.status = q.status.next();
        push_version(q);
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn update_question_note(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, note: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(q) = data.questions.iter_mut().find(|q| q.id == id) {
        q.note = note;
        push_version(q);
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

// === Flashcards CRUD ===

#[tauri::command]
pub fn add_flashcard(app: tauri::AppHandle, state: State<'_, AppState>, front: String, back: String, tag: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let fc = Flashcard {
        id: gen_id(),
        front,
        back,
        tag,
        date: today(),
    };
    data.flashcards.push(fc);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_flashcard(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    data.flashcards.retain(|f| f.id != id);
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn update_flashcard(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, front: String, back: String, tag: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    if let Some(fc) = data.flashcards.iter_mut().find(|f| f.id == id) {
        fc.front = front;
        fc.back = back;
        fc.tag = tag;
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

// === Tasks CRUD ===

#[tauri::command]
pub fn add_task(app: tauri::AppHandle, state: State<'_, AppState>, title: String, status: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let task = Task {
        id: data.next_task_id,
        title,
        priority: TaskPriority::Medium,
        date: today(),
        note: String::new(),
        subtasks: vec![],
    };
    data.next_task_id += 1;
    match status.as_str() {
        "doing" => data.tasks.doing.push(task),
        "done" => data.tasks.done.push(task),
        _ => data.tasks.todo.push(task),
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn update_task(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, status: String, title: String, priority: String, date: String, note: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let list = match status.as_str() {
        "doing" => &mut data.tasks.doing,
        "done" => &mut data.tasks.done,
        _ => &mut data.tasks.todo,
    };
    if let Some(task) = list.iter_mut().find(|t| t.id == id) {
        task.title = title;
        task.priority = match priority.as_str() {
            "high" => TaskPriority::High,
            "low" => TaskPriority::Low,
            _ => TaskPriority::Medium,
        };
        task.date = date;
        task.note = note;
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_task(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, status: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let board = &mut data.tasks;
    match status.as_str() {
        "doing" => board.doing.retain(|t| t.id != id),
        "done" => board.done.retain(|t| t.id != id),
        _ => board.todo.retain(|t| t.id != id),
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn move_task(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, from: String, to: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let board = &mut data.tasks;
    let src = match from.as_str() {
        "doing" => &mut board.doing,
        "done" => &mut board.done,
        _ => &mut board.todo,
    };
    let idx = src.iter().position(|t| t.id == id);
    if let Some(i) = idx {
        let mut task = src.remove(i);
        task.priority = TaskPriority::Medium;
        let dst = match to.as_str() {
            "doing" => &mut board.doing,
            "done" => &mut board.done,
            _ => &mut board.todo,
        };
        dst.push(task);
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn add_task_subtask(app: tauri::AppHandle, state: State<'_, AppState>, task_id: u64, status: String, text: String) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let board = &mut data.tasks;
    let list = match status.as_str() {
        "doing" => &mut board.doing,
        "done" => &mut board.done,
        _ => &mut board.todo,
    };
    if let Some(task) = list.iter_mut().find(|t| t.id == task_id) {
        task.subtasks.push(Subtask {
            id: gen_id(),
            text,
            done: false,
        });
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn toggle_task_subtask(app: tauri::AppHandle, state: State<'_, AppState>, task_id: u64, status: String, subtask_id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let board = &mut data.tasks;
    let list = match status.as_str() {
        "doing" => &mut board.doing,
        "done" => &mut board.done,
        _ => &mut board.todo,
    };
    if let Some(task) = list.iter_mut().find(|t| t.id == task_id) {
        if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
            sub.done = !sub.done;
        }
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

#[tauri::command]
pub fn delete_task_subtask(app: tauri::AppHandle, state: State<'_, AppState>, task_id: u64, status: String, subtask_id: u64) -> Result<AppData, String> {
    let mut data = state.0.lock().unwrap();
    let board = &mut data.tasks;
    let list = match status.as_str() {
        "doing" => &mut board.doing,
        "done" => &mut board.done,
        _ => &mut board.todo,
    };
    if let Some(task) = list.iter_mut().find(|t| t.id == task_id) {
        task.subtasks.retain(|s| s.id != subtask_id);
    }
    storage::save(&app, &data)?;
    Ok(data.clone())
}

// === Attachments CRUD ===

#[tauri::command]
pub fn add_attachment(app: tauri::AppHandle, state: State<'_, AppState>, name: String, data: Vec<u8>) -> Result<AppData, String> {
    use std::io::Write;
    let mut d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    std::fs::create_dir_all(&attach_dir).map_err(|e| e.to_string())?;
    let file_path = attach_dir.join(&name);
    let mut f = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    f.write_all(&data).map_err(|e| e.to_string())?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    let attachment = Attachment {
        id: gen_id(),
        filename: name,
        size: meta.len(),
        note_ids: vec![],
        folder: String::new(),
        created: now_iso(),
        date: today(),
    };
    d.attachments.push(attachment);
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn delete_attachment(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    if let Some(att) = d.attachments.iter().find(|a| a.id == id) {
        let file_path = attach_dir.join(&att.filename);
        std::fs::remove_file(&file_path).map_err(|e| format!("删除文件失败: {}", e))?;
    }
    d.attachments.retain(|a| a.id != id);
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn link_note_attachment(app: tauri::AppHandle, state: State<'_, AppState>, attachment_id: u64, note_id: u64) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    if let Some(att) = d.attachments.iter_mut().find(|a| a.id == attachment_id) {
        if !att.note_ids.contains(&note_id) {
            att.note_ids.push(note_id);
        }
    }
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn unlink_note_attachment(app: tauri::AppHandle, state: State<'_, AppState>, attachment_id: u64, note_id: u64) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    if let Some(att) = d.attachments.iter_mut().find(|a| a.id == attachment_id) {
        att.note_ids.retain(|&n| n != note_id);
    }
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn move_attachment(app: tauri::AppHandle, state: State<'_, AppState>, id: u64, folder: String) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    if let Some(att) = d.attachments.iter_mut().find(|a| a.id == id) {
        let src = attach_dir.join(&att.filename);
        let dst_dir = if folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&folder) };
        std::fs::create_dir_all(&dst_dir).map_err(|e| format!("创建目录失败: {}", e))?;
        let dst = dst_dir.join(&att.filename);
        std::fs::rename(&src, &dst).map_err(|e| format!("重命名失败: {}", e))?;
        att.folder = folder;
    }
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn open_attachment_folder(state: State<'_, AppState>, app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    let path = if id == 0 {
        attach_dir
    } else if let Some(att) = d.attachments.iter().find(|a| a.id == id) {
        if att.folder.is_empty() {
            attach_dir
        } else {
            attach_dir.join(&att.folder)
        }
    } else {
        attach_dir
    };
    std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// === File Operations ===

#[tauri::command]
pub fn read_file(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> Result<(String, Vec<u8>), String> {
    let p = std::path::PathBuf::from(&path);
    let canonical = p.canonicalize().map_err(|e| format!("路径无效: {}", e))?;
    let data_state = state.0.lock().unwrap();
    let base = std::path::PathBuf::from(get_attachment_dir(&app, &data_state));
    let base_canonical = base.canonicalize().unwrap_or(base);
    if !canonical.starts_with(&base_canonical) {
        return Err("路径越权，仅允许访问附件目录".into());
    }
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
    let data = std::fs::read(&canonical).map_err(|e| format!("读取文件失败: {}", e))?;
    Ok((name, data))
}

#[tauri::command]
pub fn delete_file(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if p.is_dir() {
        std::fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
    } else {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_file_explorer(path: String) -> Result<(), String> {
    std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// === Settings ===

#[tauri::command]
pub fn save_attachment_dir(app: tauri::AppHandle, state: State<'_, AppState>, dir: String) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.attachment_dir = dir;
    std::fs::create_dir_all(get_attachment_dir(&app, &d)).map_err(|e| format!("创建附件目录失败: {}", e))?;
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn save_ai_config(app: tauri::AppHandle, state: State<'_, AppState>, url: String, model: String, key: String) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.ai_config = AiConfig { url, model, key };
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn save_webdav_config(
    app: tauri::AppHandle, state: State<'_, AppState>,
    url: String, user: String, pass: String, path: String,
    encrypt: bool, enc_pass: String,
    sync_notes: bool, sync_summaries: bool, sync_clips: bool,
    sync_questions: bool, sync_flashcards: bool, sync_tasks: bool,
    sync_attachments: bool, sync_mode: String,
    sync_interval: i64, pull_mode: String, settings_pass: String, sync_settings: bool,
) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.webdav_config = WebdavConfig {
        url, user, pass, path,
        encrypt, enc_pass,
        sync_notes, sync_summaries, sync_clips,
        sync_questions, sync_flashcards, sync_tasks,
        sync_attachments, sync_mode,
        sync_interval, pull_mode, settings_pass, sync_settings,
    };
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn save_shortcuts(app: tauri::AppHandle, state: State<'_, AppState>, send_note: String, quick_note: String) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.shortcuts = ShortcutConfig { send_note, quick_note };
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn save_data_dir(app: tauri::AppHandle, state: State<'_, AppState>, dir: String) -> Result<DataDirResult, String> {
    let mut d = state.0.lock().unwrap();
    let old_dir = d.data_dir.clone();
    d.data_dir = dir.clone();
    storage::save(&app, &d)?;
    Ok(DataDirResult { old_dir, new_dir: dir })
}

// === Network (Async) ===

#[tauri::command]
pub async fn test_ai_connection(url: String, model: String, key: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 1,
    });
    let mut req = client.post(&url).json(&body);
    if !key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    let resp = req.send().await.map_err(|e| format!("连接失败: {}", e))?;
    let status = resp.status();
    if status.is_success() {
        Ok("AI 连接成功".into())
    } else {
        let text = resp.text().await.unwrap_or_default();
        Err(format!("服务器返回 {}: {}", status, text))
    }
}

#[tauri::command]
pub async fn list_ai_models(url: String, key: String) -> Result<Vec<String>, String> {
    let base_url = url.trim_end_matches('/').to_string();
    let models_url = format!("{}/models", base_url.trim_end_matches("/v1").trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(&models_url);
    if !key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    let resp = req.send().await.map_err(|e| format!("获取模型列表失败: {}", e))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let models = json["data"].as_array().ok_or("无法解析模型列表")?;
    let names: Vec<String> = models.iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .collect();
    Ok(names)
}

#[tauri::command]
pub async fn test_webdav(
    url: String, user: String, pass: String, path: String,
    encrypt: bool, enc_pass: String,
    sync_notes: bool, sync_summaries: bool, sync_clips: bool,
    sync_questions: bool, sync_flashcards: bool, sync_tasks: bool,
    sync_attachments: bool, sync_mode: String,
) -> Result<String, String> {
    let cfg = WebdavConfig {
        url, user, pass, path,
        encrypt, enc_pass,
        sync_notes, sync_summaries, sync_clips,
        sync_questions, sync_flashcards, sync_tasks,
        sync_attachments, sync_mode,
        sync_interval: 0,
        pull_mode: "add".into(),
        settings_pass: String::new(),
        sync_settings: false,
    };
    webdav::test_connection(&cfg).await
}

#[tauri::command]
pub async fn verify_webdav_encryption(
    url: String, user: String, pass: String, path: String,
    enc_pass: String,
) -> Result<String, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let cfg = WebdavConfig {
        encrypt: !enc_pass.is_empty(),
        enc_pass: enc_pass.clone(),
        sync_interval: 0,
        url, user, pass, path,
        sync_notes: false, sync_summaries: false, sync_clips: false,
        sync_questions: false, sync_flashcards: false, sync_tasks: false,
        sync_attachments: false, sync_mode: "upload".into(),
        pull_mode: "add".into(),
        settings_pass: String::new(),
        sync_settings: false,
    };

    let raw = webdav::download_raw(&cfg).await?;

    let try_decompress = |bytes: &[u8]| -> Result<String, String> {
        let mut decoder = GzDecoder::new(bytes);
        let mut s = String::new();
        decoder.read_to_string(&mut s).map_err(|_| "不是有效的 gzip 数据".to_string())?;
        Ok(s)
    };

    if let Ok(text) = try_decompress(&raw) {
        if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
            if !enc_pass.is_empty() {
                return Err("⚠ 远程文件是未加密的 JSON 格式，加密密码未生效".to_string());
            }
            return Ok("ℹ 远程文件是 JSON 格式（未加密）".to_string());
        }
        return Err("远程文件是 gzip 格式但内容不是有效 JSON".to_string());
    }

    if !enc_pass.is_empty() {
        let b64_str = String::from_utf8(raw).map_err(|_| "远程文件既不是 gzip 也不是有效的文本格式".to_string())?;
        let decrypted = crypto::decrypt(&b64_str, &enc_pass)?;
        if let Ok(text) = try_decompress(&decrypted) {
            if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                return Ok("✅ 加密验证通过：远程文件已使用 AES-256-GCM 加密，密码正确".to_string());
            }
        }
        return Err("⚠ 远程文件已加密且密码正确，但解密后数据格式异常".to_string());
    }

    Err("远程文件是加密格式，但未提供密码验证".to_string())
}

#[tauri::command]
pub async fn sync_webdav(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    do_sync(app, state).await
}

pub(crate) async fn do_sync(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let (cfg, json_data) = {
        let data = state.0.lock().unwrap();
        (data.webdav_config.clone(), data.clone())
    };
    if cfg.sync_mode == "merge" {
        match webdav::sync_download(&cfg).await {
            Ok(remote_compressed) => {
                app.emit("sync-progress", serde_json::json!({"progress": 20, "message": "已获取远程数据，正在解压合并..."})).ok();

                let decompressed = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    use flate2::read::GzDecoder;
                    use std::io::Read;
                    let mut decoder = GzDecoder::new(&remote_compressed[..]);
                    let mut s = String::new();
                    decoder.read_to_string(&mut s).map_err(|e| e.to_string())?;
                    Ok(s)
                }).await.map_err(|e| format!("后台解压失败: {}", e))??;

                let remote_data: AppData = serde_json::from_str(&decompressed).map_err(|e| e.to_string())?;

                let sync_data = {
                    let mut state_data = state.0.lock().unwrap();
                    app.emit("sync-progress", serde_json::json!({"progress": 40, "message": "正在合并数据..."})).ok();
                    merge_into_local(&mut state_data, &remote_data);
                    app.emit("sync-progress", serde_json::json!({"progress": 60, "message": "正在序列化合并结果..."})).ok();
                    state_data.clone()
                };

                let compressed = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                    let json = serde_json::to_vec(&sync_data).map_err(|e| e.to_string())?;
                    use flate2::write::GzEncoder;
                    use flate2::Compression;
                    use std::io::Write;
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
                    encoder.write_all(&json).map_err(|e| e.to_string())?;
                    encoder.finish().map_err(|e| e.to_string())
                }).await.map_err(|e| format!("后台压缩失败: {}", e))??;

                app.emit("sync-progress", serde_json::json!({"progress": 80, "message": "正在上传合并数据..."})).ok();

                let upload_msg = webdav::sync_upload(&cfg, &compressed).await?;

                app.emit("sync-progress", serde_json::json!({"progress": 85, "message": "正在上传设置与附件..."})).ok();
                let extras = upload_settings_and_attachments(&app, &state, &cfg).await;

                app.emit("sync-progress", serde_json::json!({"progress": 93, "message": "正在保存..."})).ok();

                let d = state.0.lock().unwrap();
                storage::save(&app, &d)?;

                let combined = format!("{}；{}", upload_msg, extras);
                app.emit("sync-progress", serde_json::json!({"progress": 100, "message": &combined})).ok();
                Ok(combined)
            }
            Err(_) => {
                app.emit("sync-progress", serde_json::json!({"progress": 40, "message": "远程无数据，执行纯上传"})).ok();

                let sync_data = json_data;

                let compressed = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                    let json = serde_json::to_vec(&sync_data).map_err(|e| e.to_string())?;
                    use flate2::write::GzEncoder;
                    use flate2::Compression;
                    use std::io::Write;
                    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
                    encoder.write_all(&json).map_err(|e| e.to_string())?;
                    encoder.finish().map_err(|e| e.to_string())
                }).await.map_err(|e| format!("后台任务失败: {}", e))??;

                app.emit("sync-progress", serde_json::json!({"progress": 60, "message": "正在上传数据..."})).ok();

                let upload_msg = webdav::sync_upload(&cfg, &compressed).await?;

                app.emit("sync-progress", serde_json::json!({"progress": 85, "message": "正在上传设置与附件..."})).ok();
                let extras = upload_settings_and_attachments(&app, &state, &cfg).await;

                app.emit("sync-progress", serde_json::json!({"progress": 93, "message": "正在保存..."})).ok();

                let d = state.0.lock().unwrap();
                storage::save(&app, &d)?;

                let combined = format!("{}；{}", upload_msg, extras);
                app.emit("sync-progress", serde_json::json!({"progress": 100, "message": &combined})).ok();
                Ok(combined)
            }
        }
    } else {
        app.emit("sync-progress", serde_json::json!({"progress": 15, "message": "正在序列化数据..."})).ok();

        let sync_data = json_data;

        let compressed = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
            let json = serde_json::to_vec(&sync_data).map_err(|e| e.to_string())?;
            use flate2::write::GzEncoder;
            use flate2::Compression;
            use std::io::Write;
            let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(&json).map_err(|e| e.to_string())?;
            encoder.finish().map_err(|e| e.to_string())
        }).await.map_err(|e| format!("后台任务失败: {}", e))??;

        app.emit("sync-progress", serde_json::json!({"progress": 50, "message": "正在上传数据..."})).ok();

        let upload_msg = webdav::sync_upload(&cfg, &compressed).await?;

        app.emit("sync-progress", serde_json::json!({"progress": 85, "message": "正在上传设置与附件..."})).ok();
        let extras = upload_settings_and_attachments(&app, &state, &cfg).await;

        app.emit("sync-progress", serde_json::json!({"progress": 93, "message": "正在保存配置..."})).ok();

        let d = state.0.lock().unwrap();
        storage::save(&app, &d)?;

        let combined = format!("{}；{}", upload_msg, extras);
        app.emit("sync-progress", serde_json::json!({"progress": 100, "message": &combined})).ok();
        Ok(combined)
    }
}

async fn upload_settings_and_attachments(app: &tauri::AppHandle, state: &State<'_, AppState>, cfg: &WebdavConfig) -> String {
    let mut msgs = Vec::new();

    let settings = {
        let data = state.0.lock().unwrap();
        SettingsData {
            ai_config: data.ai_config.clone(),
            shortcuts: data.shortcuts.clone(),
        }
    };

    if cfg.sync_settings {
        match webdav::upload_settings(cfg, &settings).await {
            Ok(msg) => msgs.push(msg),
            Err(e) => msgs.push(format!("设置上传跳过（{}）", e)),
        }
    } else {
        msgs.push("设置同步已关闭，跳过".into());
    }

    if cfg.sync_attachments {
        let (attach_dir, attachments) = {
            let data = state.0.lock().unwrap();
            (get_attachment_dir(app, &data), data.attachments.clone())
        };
        if attachments.is_empty() {
            msgs.push("无附件需要上传".into());
        } else {
            let mut att_cfg = cfg.clone();
            let path = att_cfg.path.trim_start_matches('/').trim_end_matches('/');
            att_cfg.path = format!("{}/attachments", path);
            let _ = webdav::ensure_dir(&att_cfg).await;
            let mut uploaded = 0;
            let mut skipped = 0;
            for att in &attachments {
                let local_path = attach_dir.join(&att.filename);
                if local_path.exists() {
                    match webdav::upload_file(cfg, local_path.to_str().unwrap(), &att.filename).await {
                        Ok(msg) => { msgs.push(msg); uploaded += 1; }
                        Err(e) => { msgs.push(format!("附件 {} 上传跳过（{}）", att.filename, e)); skipped += 1; }
                    }
                } else {
                    msgs.push(format!("附件 {} 本地文件缺失，跳过", att.filename));
                    skipped += 1;
                }
            }
            if uploaded == 0 && skipped > 0 {
                msgs.push("⚠ 所有附件均未能上传，请检查 WebDAV 账号密码（需使用服务商提供的专用应用密码，而非登录密码）".into());
            }
        }
    }

    msgs.join("；")
}

#[tauri::command]
pub async fn sync_pull(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let cfg = {
        let data = state.0.lock().unwrap();
        data.webdav_config.clone()
    };

    app.emit("sync-progress", serde_json::json!({"progress": 10, "message": "正在下载云端设置..."})).ok();

    // 1. Download & apply settings
    if cfg.sync_settings {
        match webdav::download_settings(&cfg).await {
        Ok(remote_settings) => {
            let mut data = state.0.lock().unwrap();
            data.ai_config = remote_settings.ai_config;
            data.shortcuts = remote_settings.shortcuts;
            app.emit("sync-progress", serde_json::json!({"progress": 30, "message": "云端设置已恢复"})).ok();
        }
        Err(e) => {
            app.emit("sync-progress", serde_json::json!({"progress": 30, "message": &format!("设置恢复跳过（{}）", e)})).ok();
        }
        }
    } else {
        app.emit("sync-progress", serde_json::json!({"progress": 30, "message": "设置同步已关闭，跳过"})).ok();
    }

    app.emit("sync-progress", serde_json::json!({"progress": 40, "message": "正在下载云端数据..."})).ok();

    // 2. Download data
    let remote_raw = match webdav::download_raw(&cfg).await {
        Ok(raw) => raw,
        Err(e) => {
            app.emit("sync-progress", serde_json::json!({"progress": 100, "message": &format!("下载失败: {}", e)})).ok();
            return Err(e);
        }
    };

    let enc_pass = cfg.enc_pass.clone();
    let decrypted = if cfg.encrypt && !enc_pass.is_empty() {
        let b64_str = String::from_utf8(remote_raw).map_err(|e| e.to_string())?;
        crypto::decrypt(&b64_str, &enc_pass)?
    } else {
        remote_raw
    };

    app.emit("sync-progress", serde_json::json!({"progress": 60, "message": "正在解压合并..."})).ok();

    let decompressed = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let mut decoder = GzDecoder::new(&decrypted[..]);
        let mut s = String::new();
        decoder.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    }).await.map_err(|e| format!("后台解压失败: {}", e))??;

    let remote_data: AppData = serde_json::from_str(&decompressed).map_err(|e| e.to_string())?;

    let pull_mode = cfg.pull_mode.clone();

    let (attach_dir, remote_attachments) = {
        let mut local_data = state.0.lock().unwrap();
        match pull_mode.as_str() {
            "overwrite" => {
                let wd = local_data.webdav_config.clone();
                let dd = local_data.data_dir.clone();
                let ad = local_data.attachment_dir.clone();
                let mut imported = apply_sync_scope(remote_data, &cfg);
                imported.webdav_config = wd;
                imported.data_dir = dd;
                imported.attachment_dir = ad;
                *local_data = imported;
            }
            _ => {
                let scoped = apply_sync_scope(remote_data, &cfg);
                add_new_only(&mut local_data.notes, &scoped.notes);
                add_new_summaries(&mut local_data.summaries, &scoped.summaries);
                add_new_only(&mut local_data.clips, &scoped.clips);
                add_new_only(&mut local_data.questions, &scoped.questions);
                add_new_only(&mut local_data.flashcards, &scoped.flashcards);
                add_new_only(&mut local_data.attachments, &scoped.attachments);
                add_new_only(&mut local_data.tasks.todo, &scoped.tasks.todo);
                add_new_only(&mut local_data.tasks.doing, &scoped.tasks.doing);
                add_new_only(&mut local_data.tasks.done, &scoped.tasks.done);
            }
        }
        app.emit("sync-progress", serde_json::json!({"progress": 70, "message": "数据已合并"})).ok();
        (get_attachment_dir(&app, &local_data), local_data.attachments.clone())
    };

    // 3. Download missing attachments
    if cfg.sync_attachments && !remote_attachments.is_empty() {
        app.emit("sync-progress", serde_json::json!({"progress": 80, "message": "正在下载缺失的附件..."})).ok();
        for att in &remote_attachments {
            let local_path = attach_dir.join(&att.filename);
            if !local_path.exists() {
                match webdav::download_file(&cfg, &att.filename, attach_dir.to_str().unwrap()).await {
                    Ok(msg) => { app.emit("sync-progress", serde_json::json!({"progress": 85, "message": &msg})).ok(); }
                    Err(e) => { app.emit("sync-progress", serde_json::json!({"progress": 85, "message": &format!("附件下载跳过（{}）", e)})).ok(); }
                }
            }
        }
    }

    app.emit("sync-progress", serde_json::json!({"progress": 95, "message": "正在保存..."})).ok();

    let d = state.0.lock().unwrap();
    storage::save(&app, &d)?;

    app.emit("sync-progress", serde_json::json!({"progress": 100, "message": "✅ 拉取完成"})).ok();
    Ok("拉取完成：云端数据已合并到本地".into())
}

pub(crate) fn start_auto_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let interval = {
                let state = app.state::<AppState>();
                let data = state.0.lock().unwrap();
                data.webdav_config.sync_interval
            };
            if interval <= 0 {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
            tokio::time::sleep(std::time::Duration::from_secs((interval * 60) as u64)).await;
            let state = app.state::<AppState>();
            let cfg = {
                let data = state.0.lock().unwrap();
                data.webdav_config.clone()
            };
            if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
                continue;
            }
            let _ = do_sync(app.clone(), state).await;
        }
    });
}

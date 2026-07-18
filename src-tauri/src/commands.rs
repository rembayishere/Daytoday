use tauri::State;
use tauri::Emitter;
use tauri::Manager;
use crate::models::*;
use crate::models::AttachmentFileStatus;
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

fn now_iso_filename() -> String {
    use chrono::Local;
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

// JS 安全整数上限：2^53 - 1 = 9007199254740991。
// 超出后 id 在 JS 侧被近似，导致按 id 查找（删除/编辑/关联）静默失效。
const MAX_SAFE_ID: u64 = 9007199254740991;

fn gen_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed) % 1000;
    // 毫秒时间戳（~1.7e12）左移 10 位容纳计数器，远小于 MAX_SAFE_ID。
    let id = (ts << 10) + counter;
    id.min(MAX_SAFE_ID)
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
    let found = data.notes.iter().any(|n| n.id == id);
    if !found {
        return Err(format!("未找到要删除的记录（id={}），可能已被删除或 id 超出安全范围", id));
    }
    // 先用快照保存，再移除内存中的记录，避免「删内存成功但落盘失败」导致状态不一致
    let snapshot = data.clone();
    data.notes.retain(|n| n.id != id);
    match storage::save(&app, &data) {
        Ok(()) => {
            app.emit("note-deleted", id).ok();
            Ok(data.clone())
        }
        Err(e) => {
            // 落盘失败则回滚内存，保证磁盘与内存一致
            *data = snapshot;
            Err(format!("删除记录失败，已取消：{}", e))
        }
    }
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

// 从磁盘任意路径添加附件（拖拽上传用）：根据 attachment_move_mode 决定复制还是移动。
// 与 read_file 不同，不限制源路径必须在附件目录内——用户拖入的本来就是外部文件。
#[tauri::command]
pub fn add_attachment_from_path(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> Result<AppData, String> {
    let p = std::path::PathBuf::from(&path);
    let canonical = p.canonicalize().map_err(|e| format!("路径无效: {}", e))?;
    if !canonical.is_file() {
        return Err(format!("不是有效文件: {}", path));
    }
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
    let move_mode = {
        let d = state.0.lock().unwrap();
        d.attachment_move_mode
    };
    if move_mode {
        // 移动模式：直接把源文件 rename 到附件目录（跨盘时退化为复制）
        let dest_dir = {
            let d = state.0.lock().unwrap();
            get_attachment_dir(&app, &d)
        };
        std::fs::create_dir_all(&dest_dir).map_err(|e| format!("创建附件目录失败: {}", e))?;
        let dest = dest_dir.join(&name);
        if dest.exists() {
            return Err(format!("附件目录已存在同名文件: {}", name));
        }
        if let Err(e) = std::fs::rename(&canonical, &dest) {
            // 跨盘/权限导致 rename 失败时退化为复制后删除源文件
            let data = std::fs::read(&canonical).map_err(|e2| format!("移动附件失败: {} / {}", e, e2))?;
            std::fs::write(&dest, &data).map_err(|e2| format!("移动附件失败: {} / {}", e, e2))?;
            let _ = std::fs::remove_file(&canonical);
        }
        // 复用 add_attachment 仅做记录写入（不再重复读文件）
        add_attachment_record_only(app, state, name)
    } else {
        // 复制模式：读取源文件字节后写入附件目录，源文件保留
        let data = std::fs::read(&canonical).map_err(|e| format!("读取文件失败: {}", e))?;
        add_attachment(app, state, name, data)
    }
}

// 仅写入附件记录，不写文件（文件已通过移动/复制就位）。用于移动模式。
fn add_attachment_record_only(app: tauri::AppHandle, state: State<'_, AppState>, name: String) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    let file_path = attach_dir.join(&name);
    let meta = std::fs::metadata(&file_path).map_err(|e| format!("读取文件信息失败: {}", e))?;
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
        let dir = if att.folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&att.folder) };
        let file_path = dir.join(&att.filename);
        // 文件可能已被外部删除（幽灵记录），缺失时忽略，仅清理记录
        if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| format!("删除文件失败: {}", e))?;
        }
    }
    d.attachments.retain(|a| a.id != id);
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn check_attachment_files(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<Vec<AttachmentFileStatus>, String> {
    let d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    let mut result = Vec::new();
    for att in &d.attachments {
        let dir = if att.folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&att.folder) };
        let file_path = dir.join(&att.filename);
        result.push(AttachmentFileStatus { id: att.id, exists: file_path.exists() });
    }
    Ok(result)
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
        let src_dir = if att.folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&att.folder) };
        let src = src_dir.join(&att.filename);
        let dst_dir = if folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&folder) };
        std::fs::create_dir_all(&dst_dir).map_err(|e| format!("创建目录失败: {}", e))?;
        let dst = dst_dir.join(&att.filename);
        // 源文件可能已丢失（幽灵记录），仅更新记录不报错
        if src.exists() {
            std::fs::rename(&src, &dst).map_err(|e| format!("移动失败: {}", e))?;
        }
        att.folder = folder.clone();
        if !folder.is_empty() && !d.folders.contains(&folder) {
            d.folders.push(folder);
        }
    }
    storage::save(&app, &d)?;
    Ok(d.clone())
}

// 新建空文件夹：创建磁盘目录并登记到 folders 列表（即使没有附件也能持久存在）
#[tauri::command]
pub fn create_attachment_folder(app: tauri::AppHandle, state: State<'_, AppState>, folder: String) -> Result<AppData, String> {
    let name = folder.trim().to_string();
    if name.is_empty() {
        return Err("文件夹名称不能为空".into());
    }
    let mut d = state.0.lock().unwrap();
    if d.folders.contains(&name) {
        return Err(format!("文件夹已存在: {}", name));
    }
    let attach_dir = get_attachment_dir(&app, &d);
    let dir = attach_dir.join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建文件夹失败: {}", e))?;
    d.folders.push(name);
    storage::save(&app, &d)?;
    Ok(d.clone())
}

// 重命名文件夹：更新 folders 列表与附件的 folder 字段，并重命名磁盘目录
#[tauri::command]
pub fn rename_attachment_folder(app: tauri::AppHandle, state: State<'_, AppState>, old_name: String, new_name: String) -> Result<AppData, String> {
    let old_name = old_name.trim().to_string();
    let new_name = new_name.trim().to_string();
    if old_name.is_empty() {
        return Err("原文件夹名不能为空".into());
    }
    if new_name.is_empty() {
        return Err("新文件夹名不能为空".into());
    }
    if old_name == new_name {
        return Ok(state.0.lock().unwrap().clone());
    }
    let mut d = state.0.lock().unwrap();
    if !d.folders.contains(&old_name) {
        return Err(format!("文件夹不存在: {}", old_name));
    }
    if d.folders.contains(&new_name) {
        return Err(format!("文件夹已存在: {}", new_name));
    }
    let attach_dir = get_attachment_dir(&app, &d);
    let old_dir = attach_dir.join(&old_name);
    let new_dir = attach_dir.join(&new_name);
    if old_dir.exists() {
        std::fs::create_dir_all(new_dir.parent().unwrap()).map_err(|e| format!("创建目录失败: {}", e))?;
        std::fs::rename(&old_dir, &new_dir).map_err(|e| format!("重命名文件夹失败: {}", e))?;
    } else {
        std::fs::create_dir_all(&new_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    d.folders = d.folders.iter().map(|f| if f == &old_name { new_name.clone() } else { f.clone() }).collect();
    for att in d.attachments.iter_mut() {
        if att.folder == old_name {
            att.folder = new_name.clone();
        }
    }
    storage::save(&app, &d)?;
    Ok(d.clone())
}

// 删除文件夹：mode = "move_root" 时资料移回未分类（folder 置空），mode = "delete" 时连同资料一起删除
#[tauri::command]
pub fn delete_attachment_folder(app: tauri::AppHandle, state: State<'_, AppState>, name: String, mode: String) -> Result<AppData, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("文件夹名不能为空".into());
    }
    let mut d = state.0.lock().unwrap();
    if !d.folders.contains(&name) {
        return Err(format!("文件夹不存在: {}", name));
    }
    let attach_dir = get_attachment_dir(&app, &d);
    let folder_dir = attach_dir.join(&name);
    if mode == "delete" {
        let to_delete: Vec<u64> = d.attachments.iter().filter(|a| a.folder == name).map(|a| a.id).collect();
        for id in to_delete {
            if let Some(att) = d.attachments.iter().find(|a| a.id == id) {
                let file_path = folder_dir.join(&att.filename);
                if file_path.exists() {
                    let _ = std::fs::remove_file(&file_path);
                }
            }
        }
        d.attachments.retain(|a| a.folder != name);
    } else {
        for att in d.attachments.iter_mut() {
            if att.folder == name {
                att.folder = String::new();
            }
        }
    }
    if folder_dir.exists() {
        let _ = std::fs::remove_dir_all(&folder_dir);
    }
    d.folders.retain(|f| f != &name);
    storage::save(&app, &d)?;
    Ok(d.clone())
}

#[tauri::command]
pub fn open_attachment_folder(state: State<'_, AppState>, app: tauri::AppHandle, id: u64) -> Result<(), String> {    let d = state.0.lock().unwrap();
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

// 按文件夹名打开资源管理器（管理面板用）
#[tauri::command]
pub fn open_attachment_folder_by_name(app: tauri::AppHandle, state: State<'_, AppState>, name: String) -> Result<(), String> {
    let d = state.0.lock().unwrap();
    let attach_dir = get_attachment_dir(&app, &d);
    let path = if name.is_empty() { attach_dir } else { attach_dir.join(&name) };
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_remote_attachments(
    state: State<'_, AppState>,
) -> Result<Vec<RemoteAttachment>, String> {
    let (cfg, local_names) = {
        let d = state.0.lock().unwrap();
        let local: Vec<String> = d.attachments.iter().map(|a| a.filename.clone()).collect();
        (d.webdav_config.clone(), local)
    };
    if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
        return Err("未配置 WebDAV，无法列出云端附件".into());
    }

    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/", base_url, path);

    // 先确保云端 attachments 目录存在，避免目录尚未创建时 PROPFIND 直接 404 报错
    let _ = webdav::ensure_attachment_dir(&cfg).await;

    let body = match webdav::propfind_dir(&cfg, &target).await {
        Ok(b) => b,
        Err(e) => return Err(format!("列出云端附件失败：{}", e)),
    };

    let parsed = webdav::parse_propfind(&body, &local_names);
    if parsed.is_empty() {
        // 解析为空时，把原始响应片段带出，便于排查（目录不存在 / 认证 / XML 命名空间等）
        let snippet = if body.len() > 800 { &body[..800] } else { &body };
        return Err(format!(
            "解析到 0 个云端附件。target={} | 原始响应(前800字): {}",
            target, snippet
        ));
    }

    Ok(parsed)
}

/// 调试用：返回 PROPFIND 的目标地址、HTTP 状态与原始 XML 响应，
/// 方便排查「查看/拉取」拿不到云端附件的问题。
#[tauri::command]
pub async fn debug_list_remote_attachments(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cfg = {
        let d = state.0.lock().unwrap();
        d.webdav_config.clone()
    };
    if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
        return Err("未配置 WebDAV（url/user/pass 为空）".into());
    }

    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/", base_url, path);

    let mkcol = webdav::ensure_attachment_dir(&cfg).await;
    let (status, body, parse_count) = match webdav::propfind_with_status(&cfg, &target).await {
        Ok((status, body)) => {
            let count = webdav::parse_propfind(&body, &[]).len();
            (status, body, count)
        }
        Err(e) => return Ok(serde_json::json!({
            "target": target,
            "mkcol": mkcol,
            "error": e,
            "parsed_count": 0,
        })),
    };

    Ok(serde_json::json!({
        "target": target,
        "mkcol": mkcol,
        "status": status,
        "parsed_count": parse_count,
        "body_len": body.len(),
        "body_preview": if body.len() > 1500 { &body[..1500] } else { &body },
        "encrypt": cfg.encrypt,
        "enc_pass_empty": cfg.enc_pass.is_empty(),
        "allow_unencrypted": cfg.allow_unencrypted_attachment,
        "sync_attachments": cfg.sync_attachments,
    }))
}

// 校验云端附件是否以加密方式上传：下载首个远程附件，尝试用加密密码解密
#[tauri::command]
pub async fn verify_attachment_encryption(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = {
        let d = state.0.lock().unwrap();
        d.webdav_config.clone()
    };
    if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
        return Err("未配置 WebDAV，无法校验".into());
    }
    if !cfg.encrypt || cfg.enc_pass.is_empty() {
        return Err("未启用加密或未设置加密密码，附件将明文上传，无法校验加密".into());
    }
    webdav::ensure_attachment_dir(&cfg).await.ok();
    // 列出云端附件，取第一个文件名
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/", base_url, path);
    let body = webdav::propfind_dir(&cfg, &target).await
        .map_err(|e| format!("列出云端附件失败：{}", e))?;
    let list = webdav::parse_propfind(&body, &[]);
    let first = match list.first() {
        Some(a) => a.filename.clone(),
        None => return Err("云端暂无附件，无法校验（请先上传附件）".into()),
    };
    match webdav::download_file_bytes(&cfg, &first).await {
        Ok(bytes) => {
            let b64_str = String::from_utf8(bytes).map_err(|_| "云端附件不是有效的文本（疑似非加密二进制）".to_string())?;
            match crypto::decrypt(&b64_str, &cfg.enc_pass, if cfg.enc_algorithm.is_empty() { crypto::default_algorithm() } else { &cfg.enc_algorithm }) {
                Ok(_) => Ok(format!("✅ 校验通过：云端附件「{}」已使用 {} 加密上传，密码正确", first, cfg.enc_algorithm)),
                Err(_) => Err(format!("⚠ 云端附件「{}」解密失败，可能未加密或密码不匹配", first)),
            }
        }
        Err(e) => Err(format!("下载云端附件失败：{}", e)),
    }
}

// 手动上传单个附件到云端（遵循加密/明文规则，与统一同步一致）
#[tauri::command]
pub async fn upload_attachment(app: tauri::AppHandle, state: State<'_, AppState>, id: u64) -> Result<String, String> {
    let (cfg, local_path, filename) = {
        let d = state.0.lock().unwrap();
        let att = d.attachments.iter().find(|a| a.id == id)
            .ok_or_else(|| "附件不存在".to_string())?;
        let attach_dir = get_attachment_dir(&app, &d);
        let dir = if att.folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&att.folder) };
        let path = dir.join(&att.filename);
        (d.webdav_config.clone(), path, att.filename.clone())
    };
    if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
        return Err("未配置 WebDAV，无法上传".into());
    }
    if !local_path.exists() {
        return Err(format!("本地文件缺失，无法上传：{}", filename));
    }
    webdav::ensure_attachment_dir(&cfg).await.ok();
    let msg = webdav::upload_file(&cfg, local_path.to_str().unwrap(), &filename).await?;
    let remote = format!("attachments/{}", filename);
    if webdav::remote_file_exists(&cfg, &remote).await {
        Ok(format!("{}（已确认云端存在）", msg))
    } else {
        Ok(format!("{}（返回成功，但服务端校验未找到，云盘可能不显示 WebDAV 上传的文件）", msg))
    }
}

// 从云端手动下载单个附件到本地（遵循加密/明文规则，与统一同步一致）
#[tauri::command]
pub async fn download_attachment(app: tauri::AppHandle, state: State<'_, AppState>, filename: String, folder: String) -> Result<AppData, String> {
    if filename.is_empty() {
        return Err("文件名不能为空".into());
    }
    let cfg = {
        let d = state.0.lock().unwrap();
        d.webdav_config.clone()
    };
    if cfg.url.is_empty() || cfg.user.is_empty() || cfg.pass.is_empty() {
        return Err("未配置 WebDAV，无法下载".into());
    }
    let attach_dir = {
        let d = state.0.lock().unwrap();
        get_attachment_dir(&app, &d)
    };
    let target_dir = if folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&folder) };
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let _msg = webdav::download_file(&cfg, &filename, target_dir.to_str().unwrap()).await?;

    // 落盘后处理本地记录：无记录则补建；有记录则更新其所属文件夹并把已有文件移到目标目录，避免重复文件
    let mut d = state.0.lock().unwrap();
    if let Some(att) = d.attachments.iter_mut().find(|a| a.filename == filename) {
        if att.folder != folder {
            let old_dir = if att.folder.is_empty() { attach_dir.clone() } else { attach_dir.join(&att.folder) };
            let old_path = old_dir.join(&filename);
            if old_path.exists() && old_path != target_dir.join(&filename) {
                let _ = std::fs::rename(&old_path, &target_dir.join(&filename));
            }
            att.folder = folder.clone();
            storage::save(&app, &d)?;
        }
    } else {
        let file_path = target_dir.join(&filename);
        let size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        d.attachments.push(Attachment {
            id: gen_id(),
            filename: filename.clone(),
            size,
            note_ids: vec![],
            folder: folder.clone(),
            created: now_iso(),
            date: today(),
        });
        if !folder.is_empty() && !d.folders.contains(&folder) {
            d.folders.push(folder.clone());
        }
        storage::save(&app, &d)?;
    }
    Ok(d.clone())
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
pub fn save_attachment_dir(app: tauri::AppHandle, state: State<'_, AppState>, dir: String) -> Result<AttachmentMigrateResult, String> {
    let mut d = state.0.lock().unwrap();
    let old_attach_dir = get_attachment_dir(&app, &d);
    let old_dir_str = old_attach_dir.to_string_lossy().to_string();
    d.attachment_dir = dir;
    let new_attach_dir = get_attachment_dir(&app, &d);
    let new_dir_str = new_attach_dir.to_string_lossy().to_string();
    std::fs::create_dir_all(&new_attach_dir).map_err(|e| format!("创建附件目录失败: {}", e))?;

    let mut result = AttachmentMigrateResult {
        old_dir: old_dir_str.clone(),
        new_dir: new_dir_str.clone(),
        moved: 0,
        skipped: 0,
        backup_dir: String::new(),
    };

    // 迁移已有附件文件：把旧目录下的文件移动到新目录，避免资料「丢失」
    if old_attach_dir != new_attach_dir && old_attach_dir.exists() {
        // 备份目录：仅当出现同名冲突时创建，用于保留旧文件以便核对
        let backup_dir = new_attach_dir.join(format!("_migration_backup_{}", now_iso_filename()));
        let mut backup_created = false;

        for att in &d.attachments {
            let src = old_attach_dir.join(&att.filename);
            if !src.exists() {
                continue;
            }
            // 目标保留子文件夹结构
            let dst_rel = if att.folder.is_empty() {
                std::path::PathBuf::from(&att.filename)
            } else {
                std::path::PathBuf::from(&att.folder).join(&att.filename)
            };
            let dst = new_attach_dir.join(&dst_rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
            }

            if dst.exists() {
                // 冲突：同名文件已存在于新目录，保留新目录文件，旧文件备份后跳过
                if !backup_created {
                    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("创建备份目录失败: {}", e))?;
                    backup_created = true;
                    result.backup_dir = backup_dir.to_string_lossy().to_string();
                }
                let backup_dst = backup_dir.join(&dst_rel);
                if let Some(parent) = backup_dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                // 备份旧文件（不覆盖新目录现有文件）
                if let Ok(bytes) = std::fs::read(&src) {
                    let _ = std::fs::write(&backup_dst, &bytes);
                }
                result.skipped += 1;
            } else {
                if let Err(e) = std::fs::rename(&src, &dst) {
                    // 跨盘/权限导致 rename 失败时退化为复制
                    if let Ok(bytes) = std::fs::read(&src) {
                        std::fs::write(&dst, &bytes).map_err(|e2| format!("迁移附件 {} 失败: {} / {}", att.filename, e, e2))?;
                        let _ = std::fs::remove_file(&src);
                    } else {
                        return Err(format!("迁移附件 {} 失败: {}", att.filename, e));
                    }
                }
                result.moved += 1;
            }
        }
    }

    storage::save(&app, &d)?;
    Ok(result)
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
    encrypt: bool, enc_pass: String, enc_algorithm: String,
    sync_notes: bool, sync_summaries: bool, sync_clips: bool,
    sync_questions: bool, sync_flashcards: bool, sync_tasks: bool,
    sync_attachments: bool, sync_mode: String,
    sync_interval: i64, pull_mode: String, settings_pass: String, sync_settings: bool,
    allow_unencrypted_attachment: bool,
) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.webdav_config = WebdavConfig {
        url, user, pass, path,
        encrypt, enc_pass, enc_algorithm,
        sync_notes, sync_summaries, sync_clips,
        sync_questions, sync_flashcards, sync_tasks,
        sync_attachments, sync_mode,
        sync_interval, pull_mode, settings_pass, sync_settings,
        allow_unencrypted_attachment,
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

#[tauri::command]
pub fn save_attachment_move_mode(app: tauri::AppHandle, state: State<'_, AppState>, mode: bool) -> Result<AppData, String> {
    let mut d = state.0.lock().unwrap();
    d.attachment_move_mode = mode;
    storage::save(&app, &d)?;
    Ok(d.clone())
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
        encrypt, enc_pass, enc_algorithm: String::new(),
        sync_notes, sync_summaries, sync_clips,
        sync_questions, sync_flashcards, sync_tasks,
        sync_attachments, sync_mode,
        sync_interval: 0,
        pull_mode: "add".into(),
        settings_pass: String::new(),
        sync_settings: false,
        allow_unencrypted_attachment: false,
    };
    webdav::test_connection(&cfg).await
}

#[tauri::command]
pub async fn verify_webdav_encryption(
    url: String, user: String, pass: String, path: String,
    enc_pass: String, enc_algorithm: String,
) -> Result<String, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let enc_alg = if enc_algorithm.is_empty() { crypto::default_algorithm().to_string() } else { enc_algorithm };

    let cfg = WebdavConfig {
        encrypt: !enc_pass.is_empty(),
        enc_pass: enc_pass.clone(),
        enc_algorithm: enc_alg.clone(),
        sync_interval: 0,
        url, user, pass, path,
        sync_notes: false, sync_summaries: false, sync_clips: false,
        sync_questions: false, sync_flashcards: false, sync_tasks: false,
        sync_attachments: false, sync_mode: "upload".into(),
        pull_mode: "add".into(),
        settings_pass: String::new(),
        sync_settings: false,
        allow_unencrypted_attachment: false,
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
        let decrypted = crypto::decrypt(&b64_str, &enc_pass, &enc_alg)?;
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
                        Ok(msg) => {
                            let remote = format!("attachments/{}", att.filename);
                            if webdav::remote_file_exists(cfg, &remote).await {
                                msgs.push(format!("{}（已确认云端存在）", msg));
                                uploaded += 1;
                            } else {
                                msgs.push(format!("附件 {} 上传返回成功，但服务端校验未找到该文件（云盘可能不显示 WebDAV 上传的文件，请用 WebDAV 客户端如 RaiDrive 查看）", att.filename));
                                skipped += 1;
                            }
                        }
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

// 从 WebDAV attachments/ 目录真实文件清单，补全本地附件记录（按 filename 并集）。
// 这样即便备份 JSON 的 attachments 数组为空/陈旧，拉取后资料面板也能显示云端附件。
async fn reconcile_attachments_from_remote(
    cfg: &WebdavConfig,
    mut attachments: Vec<Attachment>,
) -> Result<Vec<Attachment>, String> {
    webdav::ensure_attachment_dir(cfg).await.ok();
    let base_url = cfg.url.trim_end_matches('/').to_string();
    let path = cfg.path.trim_start_matches('/').trim_end_matches('/');
    let target = format!("{}/{}/attachments/", base_url, path);
    let body = match webdav::propfind_dir(cfg, &target).await {
        Ok(b) => b,
        Err(_) => return Ok(attachments), // 目录不可用时不影响主流程
    };
    let remote = webdav::parse_propfind(&body, &[]);
    // 编码无关地去重：同时比较原始文件名与 URL 解码后的文件名，
    // 避免备份 JSON 中的文件名与 WebDAV href 解码结果因编码差异导致重复。
    let mut existing: std::collections::HashSet<String> = std::collections::HashSet::new();
    for a in &attachments {
        existing.insert(a.filename.clone());
        existing.insert(webdav::decode_href(&a.filename));
    }
    for r in remote {
        let dec = webdav::decode_href(&r.filename);
        if existing.contains(&r.filename) || existing.contains(&dec) {
            continue;
        }
        attachments.push(Attachment {
            id: gen_id(),
            filename: r.filename,
            size: r.size,
            note_ids: vec![],
            folder: String::new(),
            created: String::new(),
            date: String::new(),
        });
    }
    Ok(attachments)
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
    let enc_alg = if cfg.enc_algorithm.is_empty() { crypto::default_algorithm().to_string() } else { cfg.enc_algorithm.clone() };
    let decrypted = if cfg.encrypt && !enc_pass.is_empty() {
        let b64_str = String::from_utf8(remote_raw).map_err(|e| e.to_string())?;
        crypto::decrypt(&b64_str, &enc_pass, &enc_alg)?
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

    // 1) 合并备份 JSON 数据（附件记录始终保留，sync_attachments 仅控制是否下载文件）
    let mut attachments = {
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
        local_data.attachments.clone()
    };

    // 2) 从 WebDAV attachments/ 目录真实文件清单补全记录（即使备份 JSON 的 attachments 为空也能显示）
    attachments = match reconcile_attachments_from_remote(&cfg, attachments).await {
        Ok(a) => {
            let mut local_data = state.0.lock().unwrap();
            local_data.attachments = a.clone();
            a
        }
        Err(e) => {
            app.emit("sync-progress", serde_json::json!({"progress": 72, "message": &format!("附件记录补全失败（{}），仅保留备份中的数据", e)})).ok();
            let local_data = state.0.lock().unwrap();
            local_data.attachments.clone()
        }
    };

    // 2.5) 按文件名去重，避免「备份 JSON 中的记录」与「WebDAV 目录中的文件」指向同一文件却产生两条记录
    {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut deduped: Vec<Attachment> = Vec::with_capacity(attachments.len());
        for att in attachments {
            // 归一化：原始名与 URL 解码名都纳入比较
            let norm1 = att.filename.clone();
            let norm2 = webdav::decode_href(&att.filename);
            if seen.contains(&norm1) || seen.contains(&norm2) {
                continue;
            }
            seen.insert(norm1);
            seen.insert(norm2);
            deduped.push(att);
        }
        attachments = deduped;
        let mut local_data = state.0.lock().unwrap();
        local_data.attachments = attachments.clone();
    }

    // 3. 下载缺失的附件文件（仅当 sync_attachments 开启），失败可见而非静默
    let attach_dir = {
        let d = state.0.lock().unwrap();
        get_attachment_dir(&app, &d)
    };
    let mut dl_skipped = 0;
    let dl_total = attachments.len();
    if cfg.sync_attachments {
        app.emit("sync-progress", serde_json::json!({"progress": 80, "message": "正在下载缺失的附件..."})).ok();
        for att in &attachments {
            let local_path = attach_dir.join(&att.filename);
            if !local_path.exists() {
                match webdav::download_file(&cfg, &att.filename, attach_dir.to_str().unwrap()).await {
                    Ok(msg) => { app.emit("sync-progress", serde_json::json!({"progress": 85, "message": &msg})).ok(); }
                    Err(e) => {
                        dl_skipped += 1;
                        app.emit("sync-progress", serde_json::json!({"progress": 85, "message": &format!("附件「{}」下载跳过（{}）", att.filename, e)})).ok();
                    }
                }
            }
        }
    } else {
        app.emit("sync-progress", serde_json::json!({"progress": 80, "message": "已跳过附件文件下载（「资料文件」同步未开启）"})).ok();
    }

    app.emit("sync-progress", serde_json::json!({"progress": 95, "message": "正在保存..."})).ok();

    let d = state.0.lock().unwrap();
    storage::save(&app, &d)?;

    let mut result = "拉取完成：云端数据已合并到本地".to_string();
    if cfg.sync_attachments && dl_skipped > 0 {
        result.push_str(&format!("；{} 个附件文件未下载（详见进度提示）", dl_skipped));
    } else if !cfg.sync_attachments && dl_total > 0 {
        result.push_str("；附件记录已同步，文件未下载（可在设置开启「资料文件」同步）");
    }
    app.emit("sync-progress", serde_json::json!({"progress": 100, "message": "✅ 拉取完成"})).ok();
    Ok(result)
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

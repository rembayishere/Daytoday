use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub trait Identifiable {
    fn id(&self) -> u64;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub text: String,
    pub tags: Vec<String>,
    pub time: String,
    pub date: String,
}
impl Identifiable for Note { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub title: String,
    pub content: String,
    pub tag: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub title: String,
    pub url: String,
}
impl Identifiable for Clip { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestionStatus {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "in-progress")]
    InProgress,
    #[serde(rename = "answered")]
    Answered,
}

impl QuestionStatus {
    pub fn next(&self) -> Self {
        match self {
            QuestionStatus::Open => QuestionStatus::InProgress,
            QuestionStatus::InProgress => QuestionStatus::Answered,
            QuestionStatus::Answered => QuestionStatus::Open,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            QuestionStatus::Open => "待解决",
            QuestionStatus::InProgress => "进行中",
            QuestionStatus::Answered => "已解答",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: u64,
    pub q_id: u64,
    pub timestamp: String,
    pub question: String,
    pub status: QuestionStatus,
    pub desc: String,
    pub note: String,
    pub tags: Vec<String>,
    pub change_desc: String,
}
impl Identifiable for Version { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub id: u64,
    pub text: String,
    pub done: bool,
}
impl Identifiable for Subtask { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: u64,
    pub question: String,
    pub desc: String,
    pub note: String,
    pub status: QuestionStatus,
    pub tags: Vec<String>,
    pub created: String,
    pub date: String,
    pub versions: Vec<Version>,
}
impl Identifiable for Question { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flashcard {
    pub id: u64,
    pub front: String,
    pub back: String,
    pub tag: String,
    pub date: String,
}
impl Identifiable for Flashcard { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskPriority {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub priority: TaskPriority,
    pub date: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}
impl Identifiable for Task { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBoard {
    pub todo: Vec<Task>,
    pub doing: Vec<Task>,
    pub done: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub url: String,
    pub model: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebdavConfig {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub path: String,
    pub encrypt: bool,
    pub enc_pass: String,
    #[serde(default)]
    pub enc_algorithm: String,
    #[serde(default)]
    pub sync_notes: bool,
    #[serde(default)]
    pub sync_summaries: bool,
    #[serde(default)]
    pub sync_clips: bool,
    #[serde(default)]
    pub sync_questions: bool,
    #[serde(default)]
    pub sync_flashcards: bool,
    #[serde(default)]
    pub sync_tasks: bool,
    #[serde(default)]
    pub sync_attachments: bool,
    #[serde(default)]
    pub allow_unencrypted_attachment: bool,
    #[serde(default)]
    pub sync_mode: String,
    #[serde(default)]
    pub sync_interval: i64,
    #[serde(default)]
    pub pull_mode: String,
    #[serde(default)]
    pub settings_pass: String,
    #[serde(default = "default_true")]
    pub sync_settings: bool,
}

fn default_true() -> bool {
    true
}

impl Default for WebdavConfig {
    fn default() -> Self {
        WebdavConfig {
            url: String::new(),
            user: String::new(),
            pass: String::new(),
            path: "/daytoday-backup/".into(),
            encrypt: true,
            enc_pass: String::new(),
            enc_algorithm: "aes256-gcm".into(),
            sync_notes: true,
            sync_summaries: true,
            sync_clips: true,
            sync_questions: true,
            sync_flashcards: true,
            sync_tasks: true,
            sync_attachments: true,
            allow_unencrypted_attachment: false,
            sync_mode: "upload".into(),
            sync_interval: 0,
            pull_mode: "add".into(),
            settings_pass: String::new(),
            sync_settings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: u64,
    pub filename: String,
    pub size: u64,
    pub note_ids: Vec<u64>,
    #[serde(default)]
    pub folder: String,
    pub created: String,
    pub date: String,
}
impl Identifiable for Attachment { fn id(&self) -> u64 { self.id } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAttachment {
    pub filename: String,
    pub size: u64,
    pub exists_local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentFileStatus {
    pub id: u64,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub send_note: String,
    pub quick_note: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        ShortcutConfig {
            send_note: "Ctrl+Enter".into(),
            quick_note: "Alt+Q".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDirResult {
    pub old_dir: String,
    pub new_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMigrateResult {
    pub old_dir: String,
    pub new_dir: String,
    /// 成功迁移的文件数
    pub moved: u32,
    /// 因新目录已存在同名文件而跳过的文件数（保留新目录现有文件）
    pub skipped: u32,
    /// 备份目录路径（当存在冲突时创建，供用户手动核对）
    pub backup_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub ai_config: AiConfig,
    pub shortcuts: ShortcutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppData {
    pub notes: Vec<Note>,
    pub summaries: Vec<Summary>,
    pub clips: Vec<Clip>,
    pub questions: Vec<Question>,
    pub flashcards: Vec<Flashcard>,
    pub attachments: Vec<Attachment>,
    pub tasks: TaskBoard,
    pub next_task_id: u64,
    pub next_fc_id: u64,
    pub ai_config: AiConfig,
    pub webdav_config: WebdavConfig,
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
    #[serde(default)]
    pub attachment_dir: String,
    #[serde(default)]
    pub attachment_move_mode: bool,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub data_dir: String,
}

impl Default for AppData {
    fn default() -> Self {
        let now = chrono_now();
        let today = now[..10].to_string();
        AppData {
            notes: vec![
                Note { id: 1, text: "学习 Rust 所有权机制\n核心：每个值有且只有一个所有者".into(), tags: vec!["rust".into(), "学习".into()], time: "刚刚".into(), date: today.clone() },
            ],
            summaries: vec![
                Summary { title: "Rust 学习路径".into(), content: "所有权 → 生命周期 → 异步".into(), tag: "rust".into(), count: 1 },
            ],
            clips: vec![],
            questions: vec![],
            flashcards: vec![],
            attachments: vec![],
            tasks: TaskBoard { todo: vec![], doing: vec![], done: vec![] },
            next_task_id: 1,
            next_fc_id: 1,
            ai_config: AiConfig {
                url: "http://localhost:1234/v1".into(),
                model: "qwen2.5-coder-7b-instruct".into(),
                key: String::new(),
            },
            webdav_config: WebdavConfig::default(),
            shortcuts: ShortcutConfig::default(),
            attachment_dir: String::new(),
            attachment_move_mode: false,
            folders: vec![],
            data_dir: String::new(),
        }
    }
}

pub fn merge_by_id<T: Clone + Identifiable>(local: &mut Vec<T>, remote: &[T]) {
    let ids: HashSet<u64> = local.iter().map(|x| x.id()).collect();
    for item in remote {
        if ids.contains(&item.id()) {
            if let Some(existing) = local.iter_mut().find(|x| x.id() == item.id()) {
                *existing = item.clone();
            }
        } else {
            local.push(item.clone());
        }
    }
}

pub fn merge_summaries(local: &mut Vec<Summary>, remote: &[Summary]) {
    let titles: HashSet<String> = local.iter().map(|x| x.title.clone()).collect();
    for item in remote {
        if !titles.contains(&item.title) {
            local.push(item.clone());
        }
    }
}

pub fn add_new_only<T: Clone + Identifiable>(local: &mut Vec<T>, remote: &[T]) {
    let ids: HashSet<u64> = local.iter().map(|x| x.id()).collect();
    for item in remote {
        if !ids.contains(&item.id()) {
            local.push(item.clone());
        }
    }
}

pub fn add_new_summaries(local: &mut Vec<Summary>, remote: &[Summary]) {
    let titles: HashSet<String> = local.iter().map(|x| x.title.clone()).collect();
    for item in remote {
        if !titles.contains(&item.title) {
            local.push(item.clone());
        }
    }
}

pub fn apply_sync_scope(mut data: AppData, cfg: &WebdavConfig) -> AppData {
    if !cfg.sync_notes { data.notes.clear(); }
    if !cfg.sync_summaries { data.summaries.clear(); }
    if !cfg.sync_clips { data.clips.clear(); }
    if !cfg.sync_questions { data.questions.clear(); }
    if !cfg.sync_flashcards { data.flashcards.clear(); }
    if !cfg.sync_tasks { data.tasks = TaskBoard { todo: vec![], doing: vec![], done: vec![] }; }
    // 注意：附件记录始终同步（sync_attachments 仅控制是否下载文件），故不再清空 data.attachments
    data
}

fn chrono_now() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

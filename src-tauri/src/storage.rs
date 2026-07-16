use tauri::Manager;
use std::path::PathBuf;
use crate::models::{AppData, BootstrapConfig};

fn data_dir_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("daytoday")
}

fn bootstrap_path() -> PathBuf {
    data_dir_root().join("bootstrap.json")
}

pub fn get_data_path(data_dir: &str) -> PathBuf {
    if data_dir.is_empty() {
        let dir = data_dir_root();
        std::fs::create_dir_all(&dir).ok();
        dir.join("flomo-plus.json")
    } else {
        let dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&dir).ok();
        dir.join("flomo-plus.json")
    }
}

fn read_bootstrap() -> String {
    let path = bootstrap_path();
    if !path.exists() { return String::new(); }
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<BootstrapConfig>(&content) {
            return cfg.data_dir;
        }
    }
    String::new()
}

fn write_bootstrap(data_dir: &str) {
    let path = bootstrap_path();
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    if let Ok(content) = serde_json::to_string_pretty(&BootstrapConfig { data_dir: data_dir.into() }) {
        std::fs::write(&path, content).ok();
    }
}

fn get_old_data_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let dir = app_handle.path().app_data_dir().expect("无法获取 app data 目录");
    std::fs::create_dir_all(&dir).ok();
    dir.join("flomo-plus.json")
}

fn migrate_old_data(old: &PathBuf, new: &PathBuf) {
    if !old.exists() || new.exists() { return; }
    if let Ok(content) = std::fs::read_to_string(old) {
        if serde_json::from_str::<AppData>(&content).is_ok() {
            std::fs::write(new, &content).ok();
            let bak = old.with_extension("json.bak");
            std::fs::rename(old, &bak).ok();
        }
    }
}

fn gen_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

fn migrate_large_ids(data: &mut AppData) {
    let max_safe: u64 = 9007199254740991;
    let mut changed = false;

    for note in &mut data.notes {
        if note.id > max_safe { note.id = gen_id(); changed = true; }
    }
    for clip in &mut data.clips {
        if clip.id > max_safe { clip.id = gen_id(); changed = true; }
    }
    for fc in &mut data.flashcards {
        if fc.id > max_safe { fc.id = gen_id(); changed = true; }
    }
    for q in &mut data.questions {
        if q.id > max_safe { q.id = gen_id(); changed = true; }
        for v in &mut q.versions {
            if v.id > max_safe { v.id = gen_id(); changed = true; }
            if v.q_id > max_safe { v.q_id = gen_id(); changed = true; }
        }
    }
    for task in data.tasks.todo.iter_mut().chain(data.tasks.doing.iter_mut()).chain(data.tasks.done.iter_mut()) {
        if task.id > max_safe { task.id = gen_id(); changed = true; }
    }

    if changed {
        if let Ok(json) = serde_json::to_string_pretty(data) {
            let path = get_data_path(&data.data_dir);
            std::fs::write(path, json).ok();
        }
    }
}

fn read_data_from(path: &PathBuf) -> Option<AppData> {
    if !path.exists() { return None; }
    std::fs::read_to_string(path).ok().and_then(|content| {
        serde_json::from_str::<AppData>(&content).ok()
    })
}

pub fn load(app_handle: &tauri::AppHandle) -> AppData {
    let boot_data_dir = read_bootstrap();
    let primary_path = get_data_path(&boot_data_dir);

    let old_path = get_old_data_path(app_handle);
    migrate_old_data(&old_path, &primary_path);

    let mut data = read_data_from(&primary_path).unwrap_or_else(AppData::default);

    if data.data_dir != boot_data_dir && !data.data_dir.is_empty() {
        let configured_path = get_data_path(&data.data_dir);
        if configured_path != primary_path {
            if let Some(loaded) = read_data_from(&configured_path) {
                data = loaded;
            }
        }
    }

    migrate_large_ids(&mut data);

    write_bootstrap(&data.data_dir);

    data
}

pub fn save(app_handle: &tauri::AppHandle, data: &AppData) -> Result<(), String> {
    let path = get_data_path(&data.data_dir);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    write_bootstrap(&data.data_dir);

    let old_path = get_old_data_path(app_handle);
    if old_path.exists() {
        let bak = old_path.with_extension("json.bak");
        std::fs::rename(&old_path, &bak).ok();
    }
    Ok(())
}

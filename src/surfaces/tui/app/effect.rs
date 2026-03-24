use crate::{
    domain::entry::EntryItem,
    infra::{
        db::Database,
        fs,
        providers::{
            PreviewService, PreviewSurface,
            external::{ExternalOpenProvider, SystemExternalOpenProvider},
        },
    },
    surfaces::tui::app::event::AppEvent,
};
use std::{path::PathBuf, sync::mpsc::Sender, thread};

#[derive(Debug, Clone)]
pub enum Effect {
    LoadDir {
        cwd: PathBuf,
        include_hidden: bool,
    },
    LoadPreview {
        entry: EntryItem,
        viewport_hint: usize,
    },
    SaveNote {
        path: PathBuf,
        note: String,
    },
    DeleteNote {
        path: PathBuf,
    },
    LoadBookmarks,
    JumpBookmark {
        name: String,
        include_hidden: bool,
    },
    SaveBookmark {
        name: String,
        path: PathBuf,
    },
    DeleteBookmark {
        name: String,
    },
    ToggleBookmark {
        name: String,
        path: PathBuf,
    },
    OpenExternal(PathBuf),
}

pub fn perform(effect: Effect, tx: Sender<AppEvent>) {
    match effect {
        Effect::LoadDir {
            cwd,
            include_hidden,
        } => {
            thread::spawn(move || {
                let result = Database::open_best_effort().and_then(|db| {
                    fs::load_entries(&cwd, include_hidden, &db).map(|entries| (db, entries))
                });
                match result {
                    Ok((_db, entries)) => {
                        let _ = tx.send(AppEvent::DirLoaded { cwd, entries });
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::DirFailed(err.to_string()));
                    }
                }
            });
        }
        Effect::LoadPreview {
            entry,
            viewport_hint,
        } => {
            thread::spawn(move || {
                let preview = PreviewService::default().preview_entry(
                    &entry,
                    viewport_hint,
                    PreviewSurface::Tui,
                );
                let _ = tx.send(AppEvent::PreviewLoaded {
                    path: entry.path.clone(),
                    preview,
                });
            });
        }
        Effect::SaveNote { path, note } => {
            thread::spawn(move || {
                let status = Database::open_default()
                    .and_then(|db| db.save_note(&path, &note))
                    .map(|_| format!("已保存备注: {}", path.display()))
                    .unwrap_or_else(|err| format!("保存备注失败: {err}"));
                let _ = tx.send(AppEvent::Status(status));
            });
        }
        Effect::DeleteNote { path } => {
            thread::spawn(move || {
                let status = Database::open_default()
                    .and_then(|db| db.delete_note(&path))
                    .map(|_| format!("已删除备注: {}", path.display()))
                    .unwrap_or_else(|err| format!("删除备注失败: {err}"));
                let _ = tx.send(AppEvent::Status(status));
            });
        }
        Effect::LoadBookmarks => {
            thread::spawn(move || {
                let result = Database::open_default().and_then(|db| db.list_bookmarks());
                match result {
                    Ok(bookmarks) => {
                        let _ = tx.send(AppEvent::BookmarksLoaded(bookmarks));
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Status(format!("加载书签失败: {err}")));
                    }
                }
            });
        }
        Effect::JumpBookmark {
            name,
            include_hidden,
        } => {
            thread::spawn(move || {
                match Database::open_default().and_then(|db| db.bookmark_path(&name)) {
                    Ok(path) => {
                        let result = Database::open_best_effort()
                            .and_then(|db| fs::load_entries(&path, include_hidden, &db));
                        match result {
                            Ok(entries) => {
                                let _ = tx.send(AppEvent::DirLoaded { cwd: path, entries });
                            }
                            Err(err) => {
                                let _ =
                                    tx.send(AppEvent::Status(format!("加载书签目录失败: {err}")));
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(AppEvent::Status(format!("跳转书签失败: {err}")));
                    }
                }
            });
        }
        Effect::SaveBookmark { name, path } => {
            thread::spawn(move || {
                let status = Database::open_default()
                    .and_then(|db| db.add_bookmark(&name, &path, None))
                    .map(|_| format!("已保存书签 {name}"))
                    .unwrap_or_else(|err| format!("保存书签失败: {err}"));
                let _ = tx.send(AppEvent::Status(status));
            });
        }
        Effect::DeleteBookmark { name } => {
            thread::spawn(move || {
                let status = Database::open_default()
                    .and_then(|db| db.delete_bookmark(&name))
                    .map(|_| format!("已删除书签 {name}"))
                    .unwrap_or_else(|err| format!("删除书签失败: {err}"));
                let _ = tx.send(AppEvent::Status(status));
                if let Ok(bookmarks) = Database::open_default().and_then(|db| db.list_bookmarks()) {
                    let _ = tx.send(AppEvent::BookmarksLoaded(bookmarks));
                }
            });
        }
        Effect::ToggleBookmark { name, path } => {
            thread::spawn(move || {
                let status = match Database::open_default() {
                    Ok(db) => {
                        let bookmarks = db.list_bookmarks().unwrap_or_default();
                        if bookmarks
                            .iter()
                            .any(|bookmark| bookmark.name == name && bookmark.path == path)
                        {
                            db.delete_bookmark(&name)
                                .map(|_| format!("已移除书签 {name}"))
                                .unwrap_or_else(|err| format!("移除书签失败: {err}"))
                        } else {
                            db.add_bookmark(&name, &path, None)
                                .map(|_| format!("已保存书签 {name}"))
                                .unwrap_or_else(|err| format!("保存书签失败: {err}"))
                        }
                    }
                    Err(err) => format!("打开书签数据库失败: {err}"),
                };
                let _ = tx.send(AppEvent::Status(status));
            });
        }
        Effect::OpenExternal(path) => {
            thread::spawn(move || {
                let provider = SystemExternalOpenProvider;
                let status = provider
                    .open(&path)
                    .map(|_| format!("已使用{}打开 {}", provider.name(), path.display()))
                    .unwrap_or_else(|err| format!("外部打开失败: {err}"));
                let _ = tx.send(AppEvent::Status(status));
            });
        }
    }
}

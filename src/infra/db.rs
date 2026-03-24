use crate::domain::{bookmarks::BookmarkRecord, notes::NoteRecord};
use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    env, fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

const DB_FILE: &str = ".lsz.db";
const SCHEMA_VERSION: i64 = 1;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        let path = default_db_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let existed = path.exists();
        let conn = Connection::open(path)?;
        let db = Self { conn };
        if !existed {
            db.init()?;
        }
        Ok(db)
    }

    pub fn open_ephemeral() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    pub fn open_best_effort() -> Result<Self> {
        Self::open_default().or_else(|_| Self::open_ephemeral())
    }

    fn init(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                dev INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                path TEXT NOT NULL,
                note TEXT NOT NULL,
                PRIMARY KEY (dev, inode)
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                name TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                note TEXT
            )",
            [],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(())
    }

    pub fn resolve_note(&self, path: &Path) -> Result<Option<String>> {
        let path = fs::canonicalize(path)?;
        let meta = fs::metadata(&path)?;
        let dev = meta.dev();
        let inode = meta.ino();
        let abs = path.to_string_lossy().to_string();
        let result: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT note, path FROM notes WHERE dev=?1 AND inode=?2",
                params![dev, inode],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((note, old_path)) = result {
            if old_path != abs {
                let _ = self.conn.execute(
                    "UPDATE notes SET path=?1 WHERE dev=?2 AND inode=?3",
                    params![abs, dev, inode],
                );
            }
            return Ok(Some(note));
        }

        let path_note: Option<String> = self
            .conn
            .query_row(
                "SELECT note FROM notes WHERE path=?1",
                params![abs],
                |row| row.get(0),
            )
            .optional()?;
        if path_note.is_some() {
            let _ = self.conn.execute(
                "UPDATE notes SET dev=?1, inode=?2 WHERE path=?3",
                params![dev, inode, abs],
            );
        }
        Ok(path_note)
    }

    pub fn save_note(&self, path: &Path, note: &str) -> Result<()> {
        let path = fs::canonicalize(path)?;
        let meta = fs::metadata(&path)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO notes (dev, inode, path, note) VALUES (?1, ?2, ?3, ?4)",
            params![
                meta.dev(),
                meta.ino(),
                path.to_string_lossy().to_string(),
                note
            ],
        )?;
        Ok(())
    }

    pub fn delete_note(&self, path: &Path) -> Result<()> {
        let path = fs::canonicalize(path)?;
        let meta = fs::metadata(&path)?;
        self.conn.execute(
            "DELETE FROM notes WHERE dev=?1 AND inode=?2",
            params![meta.dev(), meta.ino()],
        )?;
        Ok(())
    }

    pub fn gc_notes(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT dev, inode, path FROM notes")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut deleted = 0;
        for row in rows {
            let (dev, inode, path) = row?;
            if !Path::new(&path).exists() {
                self.conn.execute(
                    "DELETE FROM notes WHERE dev=?1 AND inode=?2",
                    params![dev, inode],
                )?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn list_notes(&self) -> Result<Vec<NoteRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, note FROM notes ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok(NoteRecord {
                path: PathBuf::from(row.get::<_, String>(0)?),
                note: row.get(1)?,
            })
        })?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }
        Ok(notes)
    }

    pub fn add_bookmark(&self, name: &str, path: &Path, note: Option<&str>) -> Result<()> {
        let path = fs::canonicalize(path)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO bookmarks (name, path, note) VALUES (?1, ?2, ?3)",
            params![name, path.to_string_lossy().to_string(), note],
        )?;
        Ok(())
    }

    pub fn delete_bookmark(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE name=?1", params![name])?;
        Ok(())
    }

    pub fn list_bookmarks(&self) -> Result<Vec<BookmarkRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path, note FROM bookmarks ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(BookmarkRecord {
                name: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                note: row.get(2)?,
            })
        })?;
        let mut bookmarks = Vec::new();
        for row in rows {
            bookmarks.push(row?);
        }
        Ok(bookmarks)
    }

    pub fn bookmark_path(&self, name: &str) -> Result<PathBuf> {
        let path: Option<String> = self
            .conn
            .query_row(
                "SELECT path FROM bookmarks WHERE name=?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        let Some(path) = path else {
            return Err(anyhow!("未找到书签: {name}"));
        };
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(anyhow!("书签路径已不存在: {}", path.display()));
        }
        Ok(path)
    }
}

pub fn default_db_path() -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("未设置 HOME 环境变量"))?;
    Ok(PathBuf::from(home).join(DB_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("lsz-db-test-{name}-{nanos}"))
    }

    #[test]
    fn save_and_resolve_note_roundtrip() {
        let file_path = unique_path("note.txt");
        File::create(&file_path).expect("create file");
        let db = Database::open_ephemeral().expect("open db");

        db.save_note(&file_path, "演示备注").expect("save note");

        assert_eq!(
            db.resolve_note(&file_path).expect("resolve note"),
            Some("演示备注".to_string())
        );
        let notes = db.list_notes().expect("list notes");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].note, "演示备注");
        assert_eq!(
            notes[0].path,
            fs::canonicalize(&file_path).expect("canonical path")
        );

        fs::remove_file(file_path).expect("cleanup");
    }

    #[test]
    fn bookmark_roundtrip() {
        let dir_path = unique_path("bookmark-dir");
        fs::create_dir_all(&dir_path).expect("create dir");
        let db = Database::open_ephemeral().expect("open db");

        db.add_bookmark("demo", &dir_path, Some("目录书签"))
            .expect("save bookmark");

        let bookmarks = db.list_bookmarks().expect("list bookmarks");
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].name, "demo");
        assert_eq!(
            db.bookmark_path("demo").expect("bookmark path"),
            fs::canonicalize(&dir_path).expect("canonical")
        );

        db.delete_bookmark("demo").expect("delete bookmark");
        assert!(db.list_bookmarks().expect("list bookmarks").is_empty());

        fs::remove_dir_all(dir_path).expect("cleanup");
    }
}

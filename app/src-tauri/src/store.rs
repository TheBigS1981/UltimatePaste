//! SQLite persistence. All user content (payload, title, OCR text, thumbnails, images)
//! is encrypted with the [`Vault`]; only timestamps, kind, flags and the source app are plain.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::crypto::Vault;
use crate::model::{now_ms, ClipContent, EntrySummary, Kind, Snippet};

const SCHEMA_VERSION: i32 = 1;

pub struct Store {
    conn: Connection,
    vault: Vault,
    images_dir: PathBuf,
}

/// A decrypted row as needed to build the search index.
pub struct IndexRow {
    pub summary: EntrySummary,
    pub content: ClipContent,
    pub ocr: Option<String>,
}

pub enum Upsert {
    Inserted(i64),
    Bumped(i64),
}

impl Upsert {
    pub fn id(&self) -> i64 {
        match self {
            Upsert::Inserted(id) | Upsert::Bumped(id) => *id,
        }
    }
}

impl Store {
    pub fn open(dir: &Path, vault: Vault) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let images_dir = dir.join("images");
        std::fs::create_dir_all(&images_dir)?;
        let conn = Connection::open(dir.join("history.db")).context("open history.db")?;
        Self::init(conn, vault, images_dir)
    }

    #[cfg(test)]
    pub fn in_memory(images_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&images_dir)?;
        Self::init(Connection::open_in_memory()?, Vault::from_secret(&[9u8; 64]), images_dir)
    }

    fn init(conn: Connection, vault: Vault, images_dir: PathBuf) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS entries(
               id INTEGER PRIMARY KEY,
               hash BLOB NOT NULL UNIQUE,
               kind TEXT NOT NULL,
               created_at INTEGER NOT NULL,
               last_used_at INTEGER NOT NULL,
               pinned INTEGER NOT NULL DEFAULT 0,
               title BLOB,
               source_app TEXT,
               payload BLOB NOT NULL,
               image_file TEXT,
               image_w INTEGER,
               image_h INTEGER,
               thumb BLOB,
               ocr BLOB,
               byte_size INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_entries_used ON entries(last_used_at);
             CREATE TABLE IF NOT EXISTS snippets(
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               keyword TEXT NOT NULL DEFAULT '',
               body BLOB NOT NULL,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );",
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn, vault, images_dir })
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Content hash used for de-duplication.
    pub fn content_hash(&self, c: &ClipContent) -> [u8; 32] {
        let files = c.files.join("\n");
        self.vault.keyed_hash(&[
            c.kind().as_str().as_bytes(),
            c.text.as_deref().unwrap_or("").as_bytes(),
            files.as_bytes(),
            c.image_png.as_deref().unwrap_or(&[]),
        ])
    }

    /// Inserts a new entry or, if identical content exists, moves it to the top.
    pub fn upsert(&mut self, c: &ClipContent, source_app: Option<&str>, thumb_png: Option<&[u8]>, image_dims: Option<(u32, u32)>) -> Result<Upsert> {
        let hash = self.content_hash(c);
        let now = now_ms();
        if let Some(id) = self
            .conn
            .query_row("SELECT id FROM entries WHERE hash=?1", [&hash[..]], |r| r.get::<_, i64>(0))
            .optional()?
        {
            self.conn.execute("UPDATE entries SET last_used_at=?1 WHERE id=?2", params![now, id])?;
            return Ok(Upsert::Bumped(id));
        }

        let image_file = match &c.image_png {
            Some(png) => {
                let name = format!("{}.bin", hex(&rand::random::<[u8; 16]>()));
                std::fs::write(self.images_dir.join(&name), self.vault.encrypt(png))?;
                Some(name)
            }
            None => None,
        };
        let payload = self.vault.encrypt(&serde_json::to_vec(c)?);
        let thumb = thumb_png.map(|t| self.vault.encrypt(t));
        self.conn.execute(
            "INSERT INTO entries(hash,kind,created_at,last_used_at,pinned,source_app,payload,image_file,image_w,image_h,thumb,byte_size)
             VALUES(?1,?2,?3,?3,0,?4,?5,?6,?7,?8,?9,?10)",
            params![
                &hash[..],
                c.kind().as_str(),
                now,
                source_app,
                payload,
                image_file,
                image_dims.map(|d| d.0),
                image_dims.map(|d| d.1),
                thumb,
                c.byte_size() as i64
            ],
        )?;
        Ok(Upsert::Inserted(self.conn.last_insert_rowid()))
    }

    pub fn load_all(&self) -> Result<Vec<IndexRow>> {
        self.load_rows(None)
    }

    /// One decrypted row (for updating the index after a change).
    pub fn load_row(&self, id: i64) -> Result<Option<IndexRow>> {
        Ok(self.load_rows(Some(id))?.into_iter().next())
    }

    pub fn find_by_hash(&self, hash: &[u8; 32]) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT id FROM entries WHERE hash=?1", [&hash[..]], |r| r.get(0)).optional()?)
    }

    fn load_rows(&self, only: Option<i64>) -> Result<Vec<IndexRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,kind,created_at,last_used_at,pinned,title,source_app,payload,image_w,image_h,ocr,byte_size,image_file
             FROM entries WHERE (?1 IS NULL OR id = ?1) ORDER BY last_used_at DESC",
        )?;
        let rows = stmt.query_map([only], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, Option<Vec<u8>>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Vec<u8>>(7)?,
                r.get::<_, Option<u32>>(8)?,
                r.get::<_, Option<u32>>(9)?,
                r.get::<_, Option<Vec<u8>>>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, Option<String>>(12)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, kind, created, used, pinned, title, source, payload, w, h, ocr, size, image_file) = row?;
            let content: ClipContent = match self.vault.decrypt(&payload).ok().and_then(|p| serde_json::from_slice(&p).ok()) {
                Some(c) => c,
                None => {
                    log::warn!("entry {id}: cannot decrypt payload, skipping");
                    continue;
                }
            };
            let title = title.and_then(|t| self.vault.decrypt_string(&t).ok());
            let ocr = ocr.and_then(|t| self.vault.decrypt_string(&t).ok());
            let mut formats = content.formats();
            if image_file.is_some() {
                formats.push("image".into());
            }
            out.push(IndexRow {
                summary: EntrySummary {
                    id,
                    kind: Kind::parse(&kind),
                    title,
                    preview: crate::model::make_preview(&content),
                    source_app: source,
                    created_at: created,
                    last_used_at: used,
                    pinned,
                    formats,
                    image_w: w,
                    image_h: h,
                    file_count: content.files.len(),
                    byte_size: size,
                    has_ocr: ocr.as_deref().is_some_and(|o| !o.trim().is_empty()),
                },
                content,
                ocr,
            });
        }
        Ok(out)
    }

    /// Loads the full content including the image.
    pub fn get_content(&self, id: i64) -> Result<Option<ClipContent>> {
        let row = self
            .conn
            .query_row("SELECT payload,image_file FROM entries WHERE id=?1", [id], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Option<String>>(1)?))
            })
            .optional()?;
        let Some((payload, image_file)) = row else { return Ok(None) };
        let mut c: ClipContent = serde_json::from_slice(&self.vault.decrypt(&payload)?)?;
        if let Some(f) = image_file {
            c.image_png = Some(self.vault.decrypt(&std::fs::read(self.images_dir.join(f))?)?);
        }
        Ok(Some(c))
    }

    pub fn get_thumb(&self, id: i64) -> Result<Option<Vec<u8>>> {
        let t: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT thumb FROM entries WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        t.map(|t| self.vault.decrypt(&t)).transpose()
    }

    pub fn get_ocr(&self, id: i64) -> Result<Option<String>> {
        let t: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT ocr FROM entries WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        t.map(|t| self.vault.decrypt_string(&t)).transpose()
    }

    pub fn set_ocr(&self, id: i64, text: &str) -> Result<()> {
        self.conn.execute("UPDATE entries SET ocr=?1 WHERE id=?2", params![self.vault.encrypt_str(text), id])?;
        Ok(())
    }

    pub fn touch(&self, id: i64) -> Result<i64> {
        let now = now_ms();
        self.conn.execute("UPDATE entries SET last_used_at=?1 WHERE id=?2", params![now, id])?;
        Ok(now)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.conn.execute("UPDATE entries SET pinned=?1 WHERE id=?2", params![pinned, id])?;
        Ok(())
    }

    pub fn rename(&self, id: i64, title: Option<&str>) -> Result<()> {
        let enc = title.filter(|t| !t.trim().is_empty()).map(|t| self.vault.encrypt_str(t.trim()));
        self.conn.execute("UPDATE entries SET title=?1 WHERE id=?2", params![enc, id])?;
        Ok(())
    }

    /// Replaces the content with edited plain text. Returns the id that now holds it
    /// (an existing identical entry wins and this one is removed).
    pub fn update_text(&mut self, id: i64, text: &str) -> Result<i64> {
        let c = ClipContent::text_only(text);
        let hash = self.content_hash(&c);
        if let Some(other) = self
            .conn
            .query_row("SELECT id FROM entries WHERE hash=?1 AND id<>?2", params![&hash[..], id], |r| r.get::<_, i64>(0))
            .optional()?
        {
            self.delete(id)?;
            self.touch(other)?;
            return Ok(other);
        }
        let old_image = self.image_file(id)?;
        self.conn.execute(
            "UPDATE entries SET hash=?1, kind=?2, payload=?3, image_file=NULL, image_w=NULL, image_h=NULL, thumb=NULL, ocr=NULL, byte_size=?4, last_used_at=?5 WHERE id=?6",
            params![&hash[..], c.kind().as_str(), self.vault.encrypt(&serde_json::to_vec(&c)?), c.byte_size() as i64, now_ms(), id],
        )?;
        if let Some(f) = old_image {
            let _ = std::fs::remove_file(self.images_dir.join(f));
        }
        Ok(id)
    }

    fn image_file(&self, id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT image_file FROM entries WHERE id=?1", [id], |r| r.get::<_, Option<String>>(0))
            .optional()?
            .flatten())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        if let Some(f) = self.image_file(id)? {
            let _ = std::fs::remove_file(self.images_dir.join(f));
        }
        self.conn.execute("DELETE FROM entries WHERE id=?1", [id])?;
        Ok(())
    }

    /// Deletes unpinned entries matching a WHERE clause; returns the deleted ids.
    fn delete_where(&self, clause: &str, value: i64) -> Result<Vec<i64>> {
        let ids: Vec<(i64, Option<String>)> = {
            let mut stmt = self.conn.prepare(&format!("SELECT id,image_file FROM entries WHERE pinned=0 AND {clause}"))?;
            let rows = stmt.query_map([value], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for (_, f) in &ids {
            if let Some(f) = f {
                let _ = std::fs::remove_file(self.images_dir.join(f));
            }
        }
        self.conn.execute(&format!("DELETE FROM entries WHERE pinned=0 AND {clause}"), [value])?;
        Ok(ids.into_iter().map(|(id, _)| id).collect())
    }

    /// Bulk delete of everything copied/used since `since_ms` (unpinned only).
    pub fn delete_since(&self, since_ms: i64) -> Result<Vec<i64>> {
        self.delete_where("last_used_at >= ?1", since_ms)
    }

    /// Retention: removes unpinned entries not used since `cutoff_ms`.
    pub fn delete_older_than(&self, cutoff_ms: i64) -> Result<Vec<i64>> {
        self.delete_where("last_used_at < ?1", cutoff_ms)
    }

    // ---------- snippets ----------

    pub fn list_snippets(&self) -> Result<Vec<Snippet>> {
        let mut stmt = self.conn.prepare("SELECT id,name,keyword,body,updated_at FROM snippets ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Vec<u8>>(3)?, r.get::<_, i64>(4)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, keyword, body, updated_at) = row?;
            if let Ok(body) = self.vault.decrypt_string(&body) {
                out.push(Snippet { id, name, keyword, body, updated_at });
            }
        }
        Ok(out)
    }

    pub fn save_snippet(&self, s: &Snippet) -> Result<i64> {
        let now = now_ms();
        let body = self.vault.encrypt_str(&s.body);
        if s.id > 0 {
            self.conn.execute(
                "UPDATE snippets SET name=?1, keyword=?2, body=?3, updated_at=?4 WHERE id=?5",
                params![s.name.trim(), s.keyword.trim(), body, now, s.id],
            )?;
            Ok(s.id)
        } else {
            self.conn.execute(
                "INSERT INTO snippets(name,keyword,body,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
                params![s.name.trim(), s.keyword.trim(), body, now],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn delete_snippet(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM snippets WHERE id=?1", [id])?;
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(tag: &str) -> Store {
        Store::in_memory(std::env::temp_dir().join(format!("up-store-{tag}-{}", std::process::id()))).unwrap()
    }

    #[test]
    fn dedupe_bumps() {
        let mut s = store("dedupe");
        let a = s.upsert(&ClipContent::text_only("hello"), Some("notepad.exe"), None, None).unwrap();
        let b = s.upsert(&ClipContent::text_only("hello"), None, None, None).unwrap();
        assert!(matches!(a, Upsert::Inserted(_)));
        assert!(matches!(b, Upsert::Bumped(id) if id == a.id()));
        assert_eq!(s.load_all().unwrap().len(), 1);
    }

    #[test]
    fn content_is_encrypted_at_rest() {
        let mut s = store("enc");
        s.upsert(&ClipContent::text_only("TOPSECRET"), None, None, None).unwrap();
        let raw: Vec<u8> = s.conn.query_row("SELECT payload FROM entries", [], |r| r.get(0)).unwrap();
        assert!(!raw.windows(9).any(|w| w == b"TOPSECRET"));
    }

    #[test]
    fn image_roundtrip_and_delete() {
        let mut s = store("img");
        let c = ClipContent { image_png: Some(vec![1, 2, 3, 4]), ..Default::default() };
        let id = s.upsert(&c, None, Some(&[9, 9]), Some((2, 2))).unwrap().id();
        assert_eq!(s.get_content(id).unwrap().unwrap().image_png.unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(s.get_thumb(id).unwrap().unwrap(), vec![9, 9]);
        s.delete(id).unwrap();
        assert!(s.get_content(id).unwrap().is_none());
    }

    #[test]
    fn pinned_survive_bulk_delete() {
        let mut s = store("bulk");
        let a = s.upsert(&ClipContent::text_only("a"), None, None, None).unwrap().id();
        let _b = s.upsert(&ClipContent::text_only("b"), None, None, None).unwrap().id();
        s.set_pinned(a, true).unwrap();
        let deleted = s.delete_since(0).unwrap();
        assert_eq!(deleted.len(), 1);
        let left = s.load_all().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].summary.id, a);
    }

    #[test]
    fn edit_rename_ocr() {
        let mut s = store("edit");
        let id = s.upsert(&ClipContent::text_only("old"), None, None, None).unwrap().id();
        s.rename(id, Some("My title")).unwrap();
        s.set_ocr(id, "ocr text").unwrap();
        let id2 = s.update_text(id, "new").unwrap();
        assert_eq!(id, id2);
        let row = s.load_row(id).unwrap().unwrap();
        assert_eq!(row.content.text.as_deref(), Some("new"));
        assert_eq!(s.find_by_hash(&s.content_hash(&ClipContent::text_only("new"))).unwrap(), Some(id));
        assert_eq!(row.summary.title.as_deref(), Some("My title"));
        // editing into an existing text merges
        let other = s.upsert(&ClipContent::text_only("other"), None, None, None).unwrap().id();
        assert_eq!(s.update_text(id, "other").unwrap(), other);
        assert_eq!(s.load_all().unwrap().len(), 1);
    }

    #[test]
    fn snippets_crud() {
        let s = store("snip");
        let id = s.save_snippet(&Snippet { id: 0, name: "Gruß".into(), keyword: "mfg".into(), body: "Viele Grüße".into(), updated_at: 0 }).unwrap();
        let list = s.list_snippets().unwrap();
        assert_eq!(list[0].body, "Viele Grüße");
        s.save_snippet(&Snippet { id, name: "Gruß".into(), keyword: "mfg".into(), body: "LG".into(), updated_at: 0 }).unwrap();
        assert_eq!(s.list_snippets().unwrap()[0].body, "LG");
        s.delete_snippet(id).unwrap();
        assert!(s.list_snippets().unwrap().is_empty());
    }
}

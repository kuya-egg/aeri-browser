//! Bookmarks: an ordered folder tree.

use rusqlite::{Connection, OptionalExtension, params};

use super::ProfileDb;
use crate::ids::BookmarkId;
use crate::store::Result;

/// A bookmark or a folder, as listed in the tree's display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub id: BookmarkId,
    pub parent: Option<BookmarkId>,
    /// 0 for top-level rows.
    pub depth: usize,
    pub title: String,
    /// `None` for a folder.
    pub url: Option<String>,
}

impl Bookmark {
    pub fn is_folder(&self) -> bool {
        self.url.is_none()
    }
}

impl ProfileDb {
    /// Every bookmark and folder, depth first in the Person's order.
    pub fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        self.conn
            .prepare(
                "WITH RECURSIVE tree (id, parent_id, title, url, depth, path) AS (
                    SELECT id, parent_id, title, url, 0, printf('%08d', position)
                      FROM bookmarks WHERE parent_id IS NULL
                    UNION ALL
                    SELECT b.id, b.parent_id, b.title, b.url, t.depth + 1,
                           t.path || '/' || printf('%08d', b.position)
                      FROM bookmarks b JOIN tree t ON b.parent_id = t.id
                 )
                 SELECT id, parent_id, title, url, depth FROM tree ORDER BY path",
            )?
            .query_map([], |row| {
                Ok(Bookmark {
                    id: BookmarkId(row.get(0)?),
                    parent: row.get::<_, Option<i64>>(1)?.map(BookmarkId),
                    title: row.get(2)?,
                    url: row.get(3)?,
                    depth: row.get::<_, i64>(4)?.max(0) as usize,
                })
            })?
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
    }

    /// Adds a bookmark (`url`) or a folder (`None`) at the end of `parent`
    /// (the top level if `None`).
    pub fn add_bookmark(
        &self,
        parent: Option<BookmarkId>,
        title: &str,
        url: Option<&str>,
    ) -> Result<BookmarkId> {
        self.conn.execute(
            "INSERT INTO bookmarks (parent_id, position, title, url) VALUES (?1,
                (SELECT COALESCE(MAX(position) + 1, 0) FROM bookmarks WHERE parent_id IS ?1),
                ?2, ?3)",
            params![parent.map(|p| p.0), title, url],
        )?;
        Ok(BookmarkId(self.conn.last_insert_rowid()))
    }

    pub fn rename_bookmark(&self, id: BookmarkId, title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE bookmarks SET title = ?2 WHERE id = ?1",
            params![id.0, title],
        )?;
        Ok(())
    }

    /// Moves a bookmark or folder to the end of `parent`. Refused (false)
    /// unless `parent` is a folder outside what moves.
    pub fn move_bookmark(&self, id: BookmarkId, parent: Option<BookmarkId>) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(parent) = parent {
            let refused: bool = tx.query_row(
                "WITH RECURSIVE moved (id) AS (
                    SELECT ?1 UNION ALL
                    SELECT b.id FROM bookmarks b JOIN moved m ON b.parent_id = m.id
                 )
                 SELECT EXISTS (SELECT 1 FROM moved WHERE id = ?2)
                     OR NOT EXISTS (SELECT 1 FROM bookmarks WHERE id = ?2 AND url IS NULL)",
                [id.0, parent.0],
                |row| row.get(0),
            )?;
            if refused {
                return Ok(false);
            }
        }
        let Some(old_parent) = tx
            .query_row(
                "SELECT parent_id FROM bookmarks WHERE id = ?1",
                [id.0],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
        else {
            return Ok(false);
        };
        tx.execute(
            "UPDATE bookmarks SET parent_id = ?2, position =
                (SELECT COALESCE(MAX(position) + 1, 0) FROM bookmarks WHERE parent_id IS ?2)
             WHERE id = ?1",
            params![id.0, parent.map(|p| p.0)],
        )?;
        renumber(&tx, old_parent)?;
        tx.commit()?;
        Ok(true)
    }

    /// Deletes a bookmark, or a folder with everything in it.
    pub fn delete_bookmark(&self, id: BookmarkId) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let parent = tx
            .query_row(
                "SELECT parent_id FROM bookmarks WHERE id = ?1",
                [id.0],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        tx.execute("DELETE FROM bookmarks WHERE id = ?1", [id.0])?;
        if let Some(parent) = parent {
            renumber(&tx, parent)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Asked on every refresh of a window's chrome, so the statement is
    /// kept prepared.
    pub fn is_bookmarked(&self, url: &str) -> Result<bool> {
        Ok(self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM bookmarks WHERE url = ?1)")?
            .query_row([url], |row| row.get(0))?)
    }

    /// ⌘D: bookmarks `url` at the end of the top level, or, if it is
    /// bookmarked anywhere, removes every bookmark of it. Returns whether
    /// it is bookmarked now.
    pub fn toggle_bookmark(&self, url: &str, title: &str) -> Result<bool> {
        if !self.is_bookmarked(url)? {
            self.add_bookmark(None, title, Some(url))?;
            return Ok(true);
        }
        let tx = self.conn.unchecked_transaction()?;
        let parents: Vec<Option<i64>> = tx
            .prepare("SELECT DISTINCT parent_id FROM bookmarks WHERE url = ?1")?
            .query_map([url], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        tx.execute("DELETE FROM bookmarks WHERE url = ?1", [url])?;
        for parent in parents {
            renumber(&tx, parent)?;
        }
        tx.commit()?;
        Ok(false)
    }
}

/// Makes a folder's positions 0, 1, 2… again after a row left it.
fn renumber(conn: &Connection, parent: Option<i64>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE bookmarks SET position = (
            SELECT COUNT(*) FROM bookmarks AS earlier
             WHERE earlier.parent_id IS bookmarks.parent_id
               AND earlier.position < bookmarks.position
         ) WHERE parent_id IS ?1",
        [parent],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_form_an_ordered_tree() {
        let db = ProfileDb::open_in_memory().unwrap();
        let a = db.add_bookmark(None, "A", Some("https://a.test/")).unwrap();
        let folder = db.add_bookmark(None, "Folder", None).unwrap();
        let b = db
            .add_bookmark(Some(folder), "B", Some("https://b.test/"))
            .unwrap();
        let c = db.add_bookmark(None, "C", Some("https://c.test/")).unwrap();
        let rows = |db: &ProfileDb| -> Vec<(BookmarkId, usize)> {
            db.bookmarks()
                .unwrap()
                .into_iter()
                .map(|b| (b.id, b.depth))
                .collect()
        };
        assert_eq!(rows(&db), [(a, 0), (folder, 0), (b, 1), (c, 0)]);

        // A moves into the folder, after B; the top level closes up.
        assert!(db.move_bookmark(a, Some(folder)).unwrap());
        assert_eq!(rows(&db), [(folder, 0), (b, 1), (a, 1), (c, 0)]);
        // C moves to the top level's end; a later add goes after it.
        assert!(db.move_bookmark(c, None).unwrap());
        let d = db.add_bookmark(None, "D", Some("https://d.test/")).unwrap();
        assert_eq!(rows(&db), [(folder, 0), (b, 1), (a, 1), (c, 0), (d, 0)]);

        // Never into itself, its own contents, or a bookmark.
        let inner = db.add_bookmark(Some(folder), "Inner", None).unwrap();
        assert!(!db.move_bookmark(folder, Some(folder)).unwrap());
        assert!(!db.move_bookmark(folder, Some(inner)).unwrap());
        assert!(!db.move_bookmark(c, Some(d)).unwrap());

        db.rename_bookmark(c, "C renamed").unwrap();
        let listed = db.bookmarks().unwrap();
        let renamed = listed.iter().find(|row| row.id == c).unwrap();
        assert_eq!(renamed.title, "C renamed");
        assert!(
            listed
                .iter()
                .find(|row| row.id == folder)
                .unwrap()
                .is_folder()
        );

        // Deleting the folder takes its contents.
        db.delete_bookmark(folder).unwrap();
        assert_eq!(rows(&db), [(c, 0), (d, 0)]);
        assert!(!db.is_bookmarked("https://a.test/").unwrap());
    }

    #[test]
    fn toggling_a_bookmark_adds_it_once_and_removes_every_copy() {
        let db = ProfileDb::open_in_memory().unwrap();
        let url = "https://a.test/";
        assert!(db.toggle_bookmark(url, "A").unwrap());
        assert!(db.is_bookmarked(url).unwrap());
        let folder = db.add_bookmark(None, "Folder", None).unwrap();
        db.add_bookmark(Some(folder), "A again", Some(url)).unwrap();
        let other = db.add_bookmark(None, "B", Some("https://b.test/")).unwrap();
        assert!(!db.toggle_bookmark(url, "A").unwrap());
        assert!(!db.is_bookmarked(url).unwrap());
        let rows: Vec<BookmarkId> = db.bookmarks().unwrap().into_iter().map(|b| b.id).collect();
        assert_eq!(rows, [folder, other]);
    }
}

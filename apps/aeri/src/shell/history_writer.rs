//! History writes off the main thread. The main thread is also CEF's UI
//! thread, so a write there delays the navigation that caused it. Each open
//! profile gets a thread with its own connection to `profile.db` (WAL lets it
//! write beside the shell's connection). Reads stay on the shell's
//! connection and see a write once this thread has made it.

use std::path::Path;
use std::sync::mpsc::{Sender, channel};

use aeri_core::profile_db::ProfileDb;

enum Write {
    Visit { url: String, title: String, at: i64 },
    Title { url: String, title: String },
}

/// Hands history writes to the profile's writer thread. Dropping it ends the
/// thread once the queued writes are done.
pub(super) struct HistoryWriter {
    tx: Sender<Write>,
}

impl HistoryWriter {
    /// Opens a second connection to the profile's database at `path`.
    pub(super) fn spawn(path: &Path) -> anyhow::Result<Self> {
        let db = ProfileDb::open(path)?;
        let (tx, rx) = channel::<Write>();
        std::thread::Builder::new()
            .name("aeri-history".into())
            .spawn(move || {
                for write in rx {
                    let written = match &write {
                        Write::Visit { url, title, at } => db.record_visit(url, title, *at),
                        Write::Title { url, title } => db.set_history_title(url, title),
                    };
                    if let Err(err) = written {
                        tracing::warn!(%err, "could not record history");
                    }
                }
            })?;
        Ok(Self { tx })
    }

    /// A finished top-level load at `at` (Unix seconds).
    pub(super) fn record_visit(&self, url: String, title: String, at: i64) {
        let _ = self.tx.send(Write::Visit { url, title, at });
    }

    /// A later title for the visit to `url`.
    pub(super) fn set_title(&self, url: String, title: String) {
        let _ = self.tx.send(Write::Title { url, title });
    }
}

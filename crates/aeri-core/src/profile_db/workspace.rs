//! A profile's workspace: spaces and their tabs, favourites, and windows.

use rusqlite::params;

use super::ProfileDb;
use crate::ids::SpaceId;
use crate::store::Result;

/// Where a window was, in screen points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A profile's whole arrangement: what reopens with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workspace {
    /// In the Person's order.
    pub spaces: Vec<SavedSpace>,
    pub favourites: Vec<SavedTab>,
    pub windows: Vec<SavedWindow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedSpace {
    pub id: SpaceId,
    pub name: String,
    /// `0xRRGGBB`.
    pub colour: u32,
    /// A glyph, or empty for none.
    pub icon: String,
    /// The space's own theme (an `.aeritheme` file), if it has one.
    pub theme: Option<String>,
    /// Pinned tabs first.
    pub tabs: Vec<SavedTab>,
    pub shown: Option<Shown>,
}

/// The tab a space shows: one of its own, or a favourite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Index into the space's tabs.
    Tab(usize),
    /// Index into the profile's favourites.
    Favourite(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedWindow {
    pub placement: Option<Placement>,
    pub space: SpaceId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedTab {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    /// The Person's own name for the tab, over the page's title.
    pub name: Option<String>,
    /// The Person's emoji or symbol for the tab.
    pub icon: Option<String>,
}

impl ProfileDb {
    /// Replaces the saved workspace.
    pub fn save_workspace(&self, workspace: &Workspace) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM windows; DELETE FROM spaces; DELETE FROM favourites;")?;
        for (position, space) in workspace.spaces.iter().enumerate() {
            let (shown_tab, shown_favourite) = match space.shown {
                Some(Shown::Tab(ix)) => (Some(ix as i64), None),
                Some(Shown::Favourite(ix)) => (None, Some(ix as i64)),
                None => (None, None),
            };
            tx.prepare_cached(
                "INSERT INTO spaces (id, position, name, colour, icon, shown_tab, shown_favourite,
                                     theme)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?
            .execute(params![
                space.id.0,
                position as i64,
                space.name,
                space.colour,
                space.icon,
                shown_tab,
                shown_favourite,
                space.theme,
            ])?;
            for (position, tab) in space.tabs.iter().enumerate() {
                tx.prepare_cached(
                    "INSERT INTO space_tabs (space_id, position, url, title, pinned, name, icon)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )?
                .execute(params![
                    space.id.0,
                    position as i64,
                    tab.url,
                    tab.title,
                    tab.pinned,
                    tab.name,
                    tab.icon,
                ])?;
            }
        }
        for (position, tab) in workspace.favourites.iter().enumerate() {
            tx.prepare_cached(
                "INSERT INTO favourites (position, url, title, name, icon)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?
            .execute(params![
                position as i64,
                tab.url,
                tab.title,
                tab.name,
                tab.icon
            ])?;
        }
        for (position, window) in workspace.windows.iter().enumerate() {
            let p = window.placement;
            tx.prepare_cached(
                "INSERT INTO windows (position, x, y, width, height, space_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![
                position as i64,
                p.map(|p| p.x),
                p.map(|p| p.y),
                p.map(|p| p.width),
                p.map(|p| p.height),
                window.space.0,
            ])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The saved workspace. Windows whose space is gone are dropped; shown
    /// tabs out of range are forgotten.
    pub fn workspace(&self) -> Result<Workspace> {
        let favourites = self
            .conn
            .prepare("SELECT url, title, 0, name, icon FROM favourites ORDER BY position")?
            .query_map([], saved_tab)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let rows = self
            .conn
            .prepare(
                "SELECT id, name, colour, icon, shown_tab, shown_favourite, theme
                 FROM spaces ORDER BY position",
            )?
            .query_map([], |row| {
                Ok((
                    SpaceId(row.get(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut tabs = self.conn.prepare(
            "SELECT url, title, pinned, name, icon FROM space_tabs
             WHERE space_id = ?1 ORDER BY position",
        )?;
        let mut spaces = Vec::with_capacity(rows.len());
        for (id, name, colour, icon, shown_tab, shown_favourite, theme) in rows {
            let tabs = tabs
                .query_map([id.0], saved_tab)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let index = |ix: Option<i64>, len: usize| {
                ix.and_then(|ix| usize::try_from(ix).ok())
                    .filter(|&ix| ix < len)
            };
            let shown = index(shown_favourite, favourites.len())
                .map(Shown::Favourite)
                .or_else(|| index(shown_tab, tabs.len()).map(Shown::Tab));
            spaces.push(SavedSpace {
                id,
                name,
                colour,
                icon,
                theme,
                tabs,
                shown,
            });
        }
        let windows = self
            .conn
            .prepare("SELECT x, y, width, height, space_id FROM windows ORDER BY position")?
            .query_map([], |row| {
                let placement = match (
                    row.get::<_, Option<f64>>(0)?,
                    row.get::<_, Option<f64>>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                ) {
                    (Some(x), Some(y), Some(width), Some(height)) => Some(Placement {
                        x,
                        y,
                        width,
                        height,
                    }),
                    _ => None,
                };
                Ok((placement, row.get::<_, Option<i64>>(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(placement, space)| {
                let space = SpaceId(space?);
                spaces
                    .iter()
                    .any(|s| s.id == space)
                    .then_some(SavedWindow { placement, space })
            })
            .collect();
        Ok(Workspace {
            spaces,
            favourites,
            windows,
        })
    }
}

/// Reads `url, title, pinned, name, icon`.
pub(super) fn saved_tab(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedTab> {
    Ok(SavedTab {
        url: row.get(0)?,
        title: row.get(1)?,
        pinned: row.get(2)?,
        name: row.get(3)?,
        icon: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::profile_db::test_support::tab;
    use crate::profile_db::{MIGRATIONS, Origin};
    use crate::store::migrate;

    fn space(id: i64, tabs: Vec<SavedTab>, shown: Option<Shown>) -> SavedSpace {
        SavedSpace {
            id: SpaceId(id),
            name: format!("Space {id}"),
            colour: 0x5b7cfa,
            icon: "★".into(),
            theme: (id == 7).then(|| r#"{"version":1}"#.to_owned()),
            tabs,
            shown,
        }
    }

    #[test]
    fn a_workspace_round_trips_and_replaces_the_previous_one() {
        let db = ProfileDb::open_in_memory().unwrap();
        assert_eq!(db.workspace().unwrap(), Workspace::default());
        let first = Workspace {
            spaces: vec![
                space(
                    7,
                    vec![tab("https://a.test/", true), tab("https://b.test/", false)],
                    Some(Shown::Tab(1)),
                ),
                space(3, vec![], Some(Shown::Favourite(0))),
            ],
            favourites: vec![tab("https://mail.test/", false)],
            windows: vec![
                SavedWindow {
                    placement: Some(Placement {
                        x: 10.0,
                        y: 20.0,
                        width: 1280.0,
                        height: 820.0,
                    }),
                    space: SpaceId(3),
                },
                SavedWindow {
                    placement: None,
                    space: SpaceId(7),
                },
            ],
        };
        db.save_workspace(&first).unwrap();
        assert_eq!(db.workspace().unwrap(), first);

        let second = Workspace {
            spaces: vec![space(1, vec![tab("https://c.test/", false)], None)],
            favourites: vec![],
            windows: vec![],
        };
        db.save_workspace(&second).unwrap();
        assert_eq!(db.workspace().unwrap(), second);
    }

    #[test]
    fn out_of_range_shown_tabs_and_windows_of_missing_spaces_are_dropped() {
        let db = ProfileDb::open_in_memory().unwrap();
        db.save_workspace(&Workspace {
            spaces: vec![
                space(1, vec![tab("https://a.test/", false)], Some(Shown::Tab(5))),
                space(2, vec![], Some(Shown::Favourite(0))),
            ],
            favourites: vec![],
            windows: vec![SavedWindow {
                placement: None,
                space: SpaceId(9),
            }],
        })
        .unwrap();
        let workspace = db.workspace().unwrap();
        assert_eq!(workspace.spaces[0].shown, None);
        assert_eq!(workspace.spaces[1].shown, None);
        assert!(workspace.windows.is_empty());
    }

    #[test]
    fn a_phase_6_session_becomes_one_space_per_window() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, &MIGRATIONS[..1]).unwrap();
        conn.execute_batch(
            "INSERT INTO windows (id, position, x, y, width, height, active_tab)
                VALUES (1, 0, 1.0, 2.0, 300.0, 400.0, 1), (2, 1, NULL, NULL, NULL, NULL, 0);
             INSERT INTO tabs VALUES (1, 0, 'https://a.test/', 'A', 1),
                (1, 1, 'https://b.test/', 'B', 0), (2, 0, 'https://c.test/', 'C', 0);
             INSERT INTO closed_tabs (url, title, pinned, position)
                VALUES ('https://d.test/', 'D', 0, 2);",
        )
        .unwrap();
        let db = ProfileDb::with_connection(conn).unwrap();
        let workspace = db.workspace().unwrap();
        let names: Vec<&str> = workspace.spaces.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Space 1", "Space 2"]);
        assert_eq!(
            workspace.spaces[0].tabs,
            vec![
                SavedTab {
                    url: "https://a.test/".into(),
                    title: "A".into(),
                    pinned: true,
                    ..SavedTab::default()
                },
                SavedTab {
                    url: "https://b.test/".into(),
                    title: "B".into(),
                    pinned: false,
                    ..SavedTab::default()
                },
            ]
        );
        assert_eq!(workspace.spaces[0].shown, Some(Shown::Tab(1)));
        assert_eq!(
            workspace.windows,
            vec![
                SavedWindow {
                    placement: Some(Placement {
                        x: 1.0,
                        y: 2.0,
                        width: 300.0,
                        height: 400.0
                    }),
                    space: workspace.spaces[0].id,
                },
                SavedWindow {
                    placement: None,
                    space: workspace.spaces[1].id,
                },
            ]
        );
        let closed = db.pop_closed().unwrap().unwrap();
        assert_eq!((closed.origin, closed.position), (Origin::Unknown, 2));
    }

    #[test]
    fn a_workspace_survives_reopening_the_file() {
        let dir = std::env::temp_dir().join(format!("aeri-profile-db-{}", std::process::id()));
        let path = dir.join("nested").join("profile.db");
        let saved = Workspace {
            spaces: vec![space(
                1,
                vec![tab("https://a.test/", false)],
                Some(Shown::Tab(0)),
            )],
            favourites: vec![],
            windows: vec![SavedWindow {
                placement: None,
                space: SpaceId(1),
            }],
        };
        ProfileDb::open(&path)
            .unwrap()
            .save_workspace(&saved)
            .unwrap();
        assert_eq!(ProfileDb::open(&path).unwrap().workspace().unwrap(), saved);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

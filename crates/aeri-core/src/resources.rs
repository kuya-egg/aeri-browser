//! Resource stewardship (Phase 11): which tabs may rest, and which rest now.
//!
//! A resting tab's page is released; its place, address and title stay, and
//! showing it loads the page again (stage 1). The engine-facing work lives
//! in the shell; the rules live here.

use std::time::Duration;

use crate::ids::TabId;
use crate::tab::Tab;

const MINUTE: Duration = Duration::from_secs(60);

/// The Person's choices for "rest tabs not shown for"; `None` is never.
pub const REST_AFTER_CHOICES: [Option<Duration>; 5] = [
    None,
    Some(Duration::from_secs(15 * 60)),
    Some(Duration::from_secs(30 * 60)),
    Some(Duration::from_secs(60 * 60)),
    Some(Duration::from_secs(4 * 60 * 60)),
];

/// Until the Person chooses: half an hour unseen.
pub const DEFAULT_REST_AFTER: Option<Duration> = Some(Duration::from_secs(30 * 60));

/// The longest a tab goes unseen before it rests while the Mac saves power
/// (Low Power Mode, or running hot).
pub const SAVING_POWER_REST_AFTER: Duration = Duration::from_secs(15 * 60);

/// Why a tab with a page stays awake (stage 1's exclusions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Awake {
    /// A window shows it.
    Shown,
    PlayingSound,
    /// Camera or microphone.
    Capturing,
    /// Typed into since it loaded: it may hold unsent input.
    Typed,
    /// Its site is marked keep-awake by the Person.
    KeptAwake,
}

/// What keeps a tab that has a page from resting, if anything.
pub fn awake_reason(tab: &Tab, shown: bool, kept_awake: bool) -> Option<Awake> {
    if shown {
        Some(Awake::Shown)
    } else if tab.activity.audible {
        Some(Awake::PlayingSound)
    } else if tab.activity.is_capturing() {
        Some(Awake::Capturing)
    } else if tab.typed {
        Some(Awake::Typed)
    } else if kept_awake {
        Some(Awake::KeptAwake)
    } else {
        None
    }
}

/// A tab that has a page and nothing keeping it awake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub tab: TabId,
    /// How long since a window last showed it.
    pub unseen_for: Duration,
}

/// The candidates unseen for at least `rest_after`.
pub fn idle_rests(candidates: &[Candidate], rest_after: Duration) -> Vec<TabId> {
    candidates
        .iter()
        .filter(|c| c.unseen_for >= rest_after)
        .map(|c| c.tab)
        .collect()
}

/// The operating system is short of memory. Least recently shown first: a
/// warning rests the older half (at least one), critical pressure all.
/// Tabs shown within the last minute are left for last and not taken on a
/// warning, so switching back and forth does not reload them.
pub fn pressure_rests(candidates: &[Candidate], critical: bool) -> Vec<TabId> {
    let mut by_age: Vec<&Candidate> = candidates.iter().collect();
    by_age.sort_by_key(|c| std::cmp::Reverse(c.unseen_for));
    if critical {
        return by_age.into_iter().map(|c| c.tab).collect();
    }
    let old: Vec<&Candidate> = by_age
        .into_iter()
        .filter(|c| c.unseen_for >= MINUTE)
        .collect();
    let take = old.len().div_ceil(2);
    old.into_iter().take(take).map(|c| c.tab).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Event;

    fn candidate(id: u64, minutes: u64) -> Candidate {
        Candidate {
            tab: TabId(id),
            unseen_for: MINUTE * minutes as u32,
        }
    }

    #[test]
    fn exclusions_keep_a_tab_awake_and_shown_comes_first() {
        let mut tab = Tab::new(TabId(1), "https://example.com/");
        assert_eq!(awake_reason(&tab, false, false), None);
        assert_eq!(awake_reason(&tab, false, true), Some(Awake::KeptAwake));
        tab.apply(&Event::Typed { tab: TabId(1) });
        assert_eq!(awake_reason(&tab, false, false), Some(Awake::Typed));
        tab.apply(&Event::MediaAccessChanged {
            tab: TabId(1),
            camera: true,
            microphone: false,
        });
        assert_eq!(awake_reason(&tab, false, false), Some(Awake::Capturing));
        tab.apply(&Event::AudibleChanged {
            tab: TabId(1),
            audible: true,
        });
        assert_eq!(awake_reason(&tab, false, false), Some(Awake::PlayingSound));
        assert_eq!(awake_reason(&tab, true, false), Some(Awake::Shown));
    }

    #[test]
    fn idle_rest_takes_only_tabs_past_the_period() {
        let candidates = [candidate(1, 59), candidate(2, 60), candidate(3, 300)];
        assert_eq!(
            idle_rests(&candidates, MINUTE * 60),
            vec![TabId(2), TabId(3)]
        );
    }

    #[test]
    fn a_warning_rests_the_older_half_and_critical_rests_all() {
        let candidates = [
            candidate(1, 5),
            candidate(2, 50),
            candidate(3, 20),
            candidate(4, 0),
        ];
        // Older than a minute: 2, 3, 1 (oldest first); half of three is two.
        assert_eq!(pressure_rests(&candidates, false), vec![TabId(2), TabId(3)]);
        assert_eq!(
            pressure_rests(&candidates, true),
            vec![TabId(2), TabId(3), TabId(1), TabId(4)]
        );
        assert!(pressure_rests(&[candidate(9, 0)], false).is_empty());
        assert_eq!(pressure_rests(&[candidate(9, 2)], false), vec![TabId(9)]);
    }
}

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use barnacle_guess::UserId;

use crate::text::Refusal;

pub const PANEL_LIFETIME_MINUTES: u64 = 15;
pub const PANEL_LIFETIME_MS: u64 = PANEL_LIFETIME_MINUTES * MILLIS_PER_MINUTE;

const MILLIS_PER_MINUTE: u64 = 60 * 1_000;

pub trait Owned {
    fn owner(&self) -> UserId;
}

#[derive(Debug, Clone)]
struct Opened<D> {
    draft: D,
    opened_at_ms: u64,
}

#[derive(Debug)]
pub struct Drafts<D> {
    open: Mutex<HashMap<u64, Opened<D>>>,
    lifetime_ms: Option<u64>,
}

impl<D> Default for Drafts<D> {
    fn default() -> Self {
        Self::lasting(Some(PANEL_LIFETIME_MS))
    }
}

impl<D> Drafts<D> {
    pub fn without_expiry() -> Self {
        Self::lasting(None)
    }

    fn lasting(lifetime_ms: Option<u64>) -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
            lifetime_ms,
        }
    }
}

impl<D: Owned + Clone> Drafts<D> {
    pub fn open(&self, panel: u64, draft: D, now_ms: u64) {
        let lifetime_ms = self.lifetime_ms;
        let mut open = self.held();
        open.retain(|_, opened| !expired(opened, lifetime_ms, now_ms));
        open.insert(
            panel,
            Opened {
                draft,
                opened_at_ms: now_ms,
            },
        );
    }

    pub fn with(&self, panel: u64, user: UserId, now_ms: u64) -> Result<D, Refusal> {
        usable(self.held().get(&panel), user, self.lifetime_ms, now_ms)
            .map(|opened| opened.draft.clone())
    }

    pub fn take(&self, panel: u64, user: UserId, now_ms: u64) -> Result<D, Refusal> {
        let mut open = self.held();
        usable(open.get(&panel), user, self.lifetime_ms, now_ms)?;
        open.remove(&panel)
            .map(|opened| opened.draft)
            .ok_or(Refusal::PanelUnknown)
    }

    pub fn replace(&self, panel: u64, draft: D) {
        if let Some(opened) = self.held().get_mut(&panel) {
            opened.draft = draft;
        }
    }

    fn held(&self) -> MutexGuard<'_, HashMap<u64, Opened<D>>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn usable<D: Owned>(
    opened: Option<&Opened<D>>,
    user: UserId,
    lifetime_ms: Option<u64>,
    now_ms: u64,
) -> Result<&Opened<D>, Refusal> {
    let opened = opened.ok_or(Refusal::PanelUnknown)?;
    if opened.draft.owner() != user {
        return Err(Refusal::PanelNotYours);
    }
    if expired(opened, lifetime_ms, now_ms) {
        return Err(Refusal::PanelExpired);
    }
    Ok(opened)
}

fn expired<D>(opened: &Opened<D>, lifetime_ms: Option<u64>, now_ms: u64) -> bool {
    lifetime_ms.is_some_and(|lifetime_ms| now_ms.saturating_sub(opened.opened_at_ms) > lifetime_ms)
}

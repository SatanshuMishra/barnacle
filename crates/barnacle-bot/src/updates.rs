use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::release_notes::Release;
use crate::updates_store::UpdateSettings;
use crate::updates_store::UpdatesStore;
use crate::updates_store::UpdatesStoreError;

#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct HeraldError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    NotSetUp,
    AlreadySent {
        version: String,
    },
    Ready {
        channel: ChannelId,
        ping: Option<Ping>,
    },
}

#[derive(Debug)]
pub enum SendOutcome {
    Sent { channel: ChannelId },
    NotSetUp,
    AlreadySent { version: String },
    VersionChanged { running: String },
    PostFailed(HeraldError),
    Storage(UpdatesStoreError),
}

pub trait Herald: Send + Sync + 'static {
    fn post(
        &self,
        channel: ChannelId,
        release: &Release,
        ping: Option<Ping>,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;
}

pub struct Updates<H: Herald> {
    store: UpdatesStore,
    herald: H,
    release: Release,
    guild_locks: std::sync::Mutex<HashMap<GuildId, Arc<tokio::sync::Mutex<()>>>>,
}

impl<H: Herald> Updates<H> {
    pub fn new(store: UpdatesStore, herald: H, release: Release) -> Arc<Self> {
        Arc::new(Self {
            store,
            herald,
            release,
            guild_locks: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn release(&self) -> &Release {
        &self.release
    }

    pub async fn configure(
        &self,
        guild: GuildId,
        channel: ChannelId,
        ping: Option<Ping>,
    ) -> Result<(), UpdatesStoreError> {
        self.store.save(guild, channel, ping).await
    }

    pub async fn preview(&self, guild: GuildId) -> Result<Preview, UpdatesStoreError> {
        let settings = self.store.settings(guild).await?;
        Ok(self.judge(settings))
    }

    pub async fn send(&self, guild: GuildId, version: &str) -> SendOutcome {
        if version != self.release.version {
            return SendOutcome::VersionChanged {
                running: self.release.version.clone(),
            };
        }
        let lock = self.guild_lock(guild);
        let _sending = lock.lock().await;
        let settings = match self.store.settings(guild).await {
            Ok(settings) => settings,
            Err(error) => return SendOutcome::Storage(error),
        };
        let (channel, ping) = match self.judge(settings) {
            Preview::NotSetUp => return SendOutcome::NotSetUp,
            Preview::AlreadySent { version } => return SendOutcome::AlreadySent { version },
            Preview::Ready { channel, ping } => (channel, ping),
        };
        if let Err(error) = self.herald.post(channel, &self.release, ping).await {
            return SendOutcome::PostFailed(error);
        }
        match self.store.mark_sent(guild, &self.release.version).await {
            Ok(()) => SendOutcome::Sent { channel },
            Err(error) => SendOutcome::Storage(error),
        }
    }

    fn judge(&self, settings: Option<UpdateSettings>) -> Preview {
        match settings {
            None => Preview::NotSetUp,
            Some(settings)
                if settings.last_version.as_deref() == Some(self.release.version.as_str()) =>
            {
                Preview::AlreadySent {
                    version: self.release.version.clone(),
                }
            }
            Some(settings) => Preview::Ready {
                channel: settings.channel,
                ping: settings.ping,
            },
        }
    }

    fn guild_lock(&self, guild: GuildId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .guild_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(locks.entry(guild).or_default())
    }
}

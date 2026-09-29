mod common;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::release_notes::Feature;
use barnacle_bot::release_notes::Release;
use barnacle_bot::updates::Herald;
use barnacle_bot::updates::HeraldError;
use barnacle_bot::updates::Preview;
use barnacle_bot::updates::SendOutcome;
use barnacle_bot::updates::Updates;
use barnacle_bot::updates_store::UpdatesStore;
use common::attendance_pool;

const VOICE_ROOMS: &str = include_str!("../../../migrations/0005_voice_rooms.sql");
const BOT_UPDATES: &str = include_str!("../../../migrations/0007_bot_updates.sql");
const CLI_DIRECTIVE: &str = ".bail on\n";

const RUNNING: &str = "0.2.0";
const OLDER: &str = "0.1.0";
const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const CHANNEL: ChannelId = ChannelId::new(10);
const OTHER_CHANNEL: ChannelId = ChannelId::new(20);
const ROLE: RoleId = RoleId::new(123);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Post {
    channel: ChannelId,
    version: String,
    ping: Option<Ping>,
}

#[derive(Default)]
struct HeraldState {
    posts: Mutex<Vec<Post>>,
    fail: AtomicBool,
}

#[derive(Clone, Default)]
struct FakeHerald {
    state: Arc<HeraldState>,
}

impl FakeHerald {
    fn fail(&self, fail: bool) {
        self.state.fail.store(fail, SeqCst);
    }

    fn posts(&self) -> Vec<Post> {
        self.state.posts.lock().unwrap().clone()
    }
}

impl Herald for FakeHerald {
    async fn post(
        &self,
        channel: ChannelId,
        release: &Release,
        ping: Option<Ping>,
    ) -> Result<(), HeraldError> {
        tokio::task::yield_now().await;
        if self.state.fail.load(SeqCst) {
            return Err(HeraldError("the fake Discord refuses every post".into()));
        }
        self.state.posts.lock().unwrap().push(Post {
            channel,
            version: release.version.clone(),
            ping,
        });
        Ok(())
    }
}

fn release(version: &str) -> Release {
    Release {
        version: version.to_owned(),
        summary: "You can now play a series of rounds.".to_owned(),
        features: vec![Feature {
            title: "Silhouette series".to_owned(),
            story: vec!["You can now play a series of rounds.".to_owned()],
            steps: vec!["Type /guess-series.".to_owned()],
            ..Feature::default()
        }],
        changes: Vec::new(),
        fixes: Vec::new(),
    }
}

async fn store() -> UpdatesStore {
    let pool = attendance_pool().await;
    for migration in [VOICE_ROOMS, BOT_UPDATES] {
        sqlx::raw_sql(migration.trim_start_matches(CLI_DIRECTIVE))
            .execute(&pool)
            .await
            .unwrap();
    }
    UpdatesStore::with_pool(pool).await.unwrap()
}

fn post(channel: ChannelId, ping: Option<Ping>) -> Post {
    Post {
        channel,
        version: RUNNING.to_owned(),
        ping,
    }
}

async fn last_version(store: &UpdatesStore, guild: GuildId) -> Option<String> {
    store
        .settings(guild)
        .await
        .unwrap()
        .and_then(|settings| settings.last_version)
}

#[tokio::test]
async fn announce_refuses_a_server_without_an_updates_channel() {
    let herald = FakeHerald::default();
    let updates = Updates::new(store().await, herald.clone(), release(RUNNING));
    updates
        .configure(OTHER_GUILD, OTHER_CHANNEL, None)
        .await
        .unwrap();
    assert_eq!(updates.preview(GUILD).await.unwrap(), Preview::NotSetUp);
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::NotSetUp
    ));
    assert_eq!(herald.posts(), Vec::new());
}

#[tokio::test]
async fn announce_skips_a_server_that_already_has_this_version() {
    let herald = FakeHerald::default();
    let store = store().await;
    store
        .save(GUILD, CHANNEL, Some(Ping::Role(ROLE)))
        .await
        .unwrap();
    store.mark_sent(GUILD, RUNNING).await.unwrap();
    store.save(OTHER_GUILD, OTHER_CHANNEL, None).await.unwrap();
    store.mark_sent(OTHER_GUILD, OLDER).await.unwrap();
    let updates = Updates::new(store, herald.clone(), release(RUNNING));
    assert_eq!(
        updates.preview(GUILD).await.unwrap(),
        Preview::AlreadySent {
            version: RUNNING.to_owned()
        }
    );
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::AlreadySent { version } if version == RUNNING
    ));
    assert_eq!(herald.posts(), Vec::new());
    assert_eq!(
        updates.preview(OTHER_GUILD).await.unwrap(),
        Preview::Ready {
            channel: OTHER_CHANNEL,
            ping: None
        }
    );
}

#[tokio::test]
async fn send_posts_once_with_the_ping_and_records_the_version() {
    let herald = FakeHerald::default();
    let store = store().await;
    let updates = Updates::new(store.clone(), herald.clone(), release(RUNNING));
    updates
        .configure(GUILD, CHANNEL, Some(Ping::Role(ROLE)))
        .await
        .unwrap();
    assert_eq!(
        updates.preview(GUILD).await.unwrap(),
        Preview::Ready {
            channel: CHANNEL,
            ping: Some(Ping::Role(ROLE))
        }
    );
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::Sent { channel } if channel == CHANNEL
    ));
    assert_eq!(herald.posts(), vec![post(CHANNEL, Some(Ping::Role(ROLE)))]);
    assert_eq!(last_version(&store, GUILD).await.as_deref(), Some(RUNNING));
    assert_eq!(
        updates.preview(GUILD).await.unwrap(),
        Preview::AlreadySent {
            version: RUNNING.to_owned()
        }
    );
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::AlreadySent { .. }
    ));
    assert_eq!(herald.posts().len(), 1);
}

#[tokio::test]
async fn a_failed_post_is_not_recorded_as_sent() {
    let herald = FakeHerald::default();
    let store = store().await;
    let updates = Updates::new(store.clone(), herald.clone(), release(RUNNING));
    updates.configure(GUILD, CHANNEL, None).await.unwrap();
    herald.fail(true);
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::PostFailed(_)
    ));
    assert_eq!(last_version(&store, GUILD).await, None);
    assert_eq!(
        updates.preview(GUILD).await.unwrap(),
        Preview::Ready {
            channel: CHANNEL,
            ping: None
        }
    );
    herald.fail(false);
    assert!(matches!(
        updates.send(GUILD, RUNNING).await,
        SendOutcome::Sent { .. }
    ));
    assert_eq!(herald.posts(), vec![post(CHANNEL, None)]);
    assert_eq!(last_version(&store, GUILD).await.as_deref(), Some(RUNNING));
}

#[tokio::test]
async fn two_quick_sends_post_once() {
    let herald = FakeHerald::default();
    let updates = Updates::new(store().await, herald.clone(), release(RUNNING));
    updates
        .configure(GUILD, CHANNEL, Some(Ping::Everyone))
        .await
        .unwrap();
    let (first, second) = tokio::join!(updates.send(GUILD, RUNNING), updates.send(GUILD, RUNNING));
    let outcomes = [first, second];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, SendOutcome::Sent { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, SendOutcome::AlreadySent { .. }))
            .count(),
        1
    );
    assert_eq!(herald.posts(), vec![post(CHANNEL, Some(Ping::Everyone))]);
}

#[tokio::test]
async fn a_send_for_a_different_version_is_refused() {
    let herald = FakeHerald::default();
    let store = store().await;
    let updates = Updates::new(store.clone(), herald.clone(), release(RUNNING));
    updates.configure(GUILD, CHANNEL, None).await.unwrap();
    assert!(matches!(
        updates.send(GUILD, OLDER).await,
        SendOutcome::VersionChanged { running } if running == RUNNING
    ));
    assert_eq!(herald.posts(), Vec::new());
    assert_eq!(last_version(&store, GUILD).await, None);
}

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;

use barnacle_bot::failure;
use barnacle_bot::failure::DiscordRefusal;
use barnacle_bot::failure::Failure;
use barnacle_bot::failure::Kind;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use barnacle_bot::voice::EMPTY_GRACE_MS;
use barnacle_bot::voice::HUB_NAME_LIMIT;
use barnacle_bot::voice::HubLayout;
use barnacle_bot::voice::JoinOutcome;
use barnacle_bot::voice::MoveOutcome;
use barnacle_bot::voice::NEW_CHANNEL_GRACE_MS;
use barnacle_bot::voice::NOTICE_INTERVAL_MS;
use barnacle_bot::voice::NameProblem;
use barnacle_bot::voice::Occupancy;
use barnacle_bot::voice::Overwrite;
use barnacle_bot::voice::OverwriteTarget;
use barnacle_bot::voice::ROOM_ACCESS;
use barnacle_bot::voice::ROOM_NAME_LIMIT;
use barnacle_bot::voice::RoomRemoval;
use barnacle_bot::voice::RoomSpec;
use barnacle_bot::voice::Rooms;
use barnacle_bot::voice::RoomsError;
use barnacle_bot::voice::SweepReport;
use barnacle_bot::voice::VoiceRooms;
use barnacle_bot::voice::clean_name;
use barnacle_bot::voice::room_channel_name;
use barnacle_bot::voice::with_room_access;
use barnacle_bot::voice_store::Hub;
use barnacle_bot::voice_store::Room;
use barnacle_bot::voice_store::VoiceStore;
use barnacle_bot::voice_text;
use barnacle_guess::UserId;
use common::attendance_pool;
use poise::serenity_prelude::Permissions;

const VOICE_ROOMS: &str = include_str!("../../../migrations/0005_voice_rooms.sql");
const CLI_DIRECTIVE: &str = ".bail on\n";

const GUILD: GuildId = GuildId::new(1);
const OTHER_GUILD: GuildId = GuildId::new(2);
const HUB: ChannelId = ChannelId::new(10);
const SECOND_HUB: ChannelId = ChannelId::new(11);
const OTHER_HUB: ChannelId = ChannelId::new(20);
const CATEGORY: ChannelId = ChannelId::new(30);
const ADMIN: UserId = UserId::new(100);
const AKI: UserId = UserId::new(200);
const BOREALIS: UserId = UserId::new(300);
const BARNACLE: UserId = UserId::new(900);
const CREWMATES: RoleId = RoleId::new(4242);
const EVERYONE: RoleId = RoleId::new(1);
const FIRST_ROOM: u64 = 5_000;
const NOW: u64 = 1_700_000_000_000;
const BLOCKERS: [&str; 2] = ["Manage Channels", "Move Members"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum RoomsCall {
    Created {
        guild: GuildId,
        spec: RoomSpec,
    },
    Moved {
        user: UserId,
        room: ChannelId,
    },
    Deleted(ChannelId),
    Notified {
        hub: ChannelId,
        user: UserId,
        content: String,
    },
}

#[derive(Default)]
struct RoomsState {
    calls: Mutex<Vec<RoomsCall>>,
    next_room: AtomicU64,
    absent: AtomicBool,
    fail_creates: AtomicBool,
    refuse_creates: Mutex<Option<DiscordRefusal>>,
    fail_moves: AtomicBool,
    fail_deletes: AtomicBool,
    fail_notices: AtomicBool,
}

#[derive(Clone)]
struct FakeRooms {
    state: Arc<RoomsState>,
}

impl FakeRooms {
    fn new() -> Self {
        let state = RoomsState::default();
        state.next_room.store(FIRST_ROOM, SeqCst);
        Self {
            state: Arc::new(state),
        }
    }

    fn answer_moves_not_in_voice(&self, absent: bool) {
        self.state.absent.store(absent, SeqCst);
    }

    fn fail_creates(&self, fail: bool) {
        self.state.fail_creates.store(fail, SeqCst);
    }

    fn refuse_creates(&self, refusal: DiscordRefusal) {
        *self.state.refuse_creates.lock().unwrap() = Some(refusal);
    }

    fn fail_notices(&self, fail: bool) {
        self.state.fail_notices.store(fail, SeqCst);
    }

    fn fail_moves(&self, fail: bool) {
        self.state.fail_moves.store(fail, SeqCst);
    }

    fn fail_deletes(&self, fail: bool) {
        self.state.fail_deletes.store(fail, SeqCst);
    }

    fn calls(&self) -> Vec<RoomsCall> {
        self.state.calls.lock().unwrap().clone()
    }

    fn created(&self) -> Vec<RoomSpec> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RoomsCall::Created { spec, .. } => Some(spec),
                _ => None,
            })
            .collect()
    }

    fn moves(&self) -> Vec<(UserId, ChannelId)> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RoomsCall::Moved { user, room } => Some((user, room)),
                _ => None,
            })
            .collect()
    }

    fn deleted(&self) -> Vec<ChannelId> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RoomsCall::Deleted(room) => Some(room),
                _ => None,
            })
            .collect()
    }

    fn notices(&self) -> Vec<(ChannelId, UserId, String)> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                RoomsCall::Notified { hub, user, content } => Some((hub, user, content)),
                _ => None,
            })
            .collect()
    }

    fn record(&self, call: RoomsCall) {
        self.state.calls.lock().unwrap().push(call);
    }
}

impl Rooms for FakeRooms {
    async fn create_room(&self, guild: GuildId, spec: &RoomSpec) -> Result<ChannelId, RoomsError> {
        tokio::task::yield_now().await;
        self.record(RoomsCall::Created {
            guild,
            spec: spec.clone(),
        });
        if self.state.fail_creates.load(SeqCst) {
            return Err(RoomsError("the fake Discord refuses every room".into()));
        }
        if let Some(refusal) = self.state.refuse_creates.lock().unwrap().clone() {
            return Err(RoomsError(Box::new(refusal)));
        }
        Ok(ChannelId::new(self.state.next_room.fetch_add(1, SeqCst)))
    }

    async fn move_member(
        &self,
        _guild: GuildId,
        user: UserId,
        room: ChannelId,
    ) -> Result<MoveOutcome, RoomsError> {
        self.record(RoomsCall::Moved { user, room });
        if self.state.fail_moves.load(SeqCst) {
            return Err(RoomsError("the fake Discord refuses every move".into()));
        }
        if self.state.absent.load(SeqCst) {
            Ok(MoveOutcome::NotInVoice)
        } else {
            Ok(MoveOutcome::Moved)
        }
    }

    async fn delete_room(&self, room: ChannelId) -> Result<RoomRemoval, RoomsError> {
        self.record(RoomsCall::Deleted(room));
        if self.state.fail_deletes.load(SeqCst) {
            return Err(RoomsError("the fake Discord refuses every delete".into()));
        }
        Ok(RoomRemoval::Deleted)
    }

    async fn notify(&self, hub: ChannelId, user: UserId, content: &str) -> Result<(), RoomsError> {
        self.record(RoomsCall::Notified {
            hub,
            user,
            content: content.to_owned(),
        });
        if self.state.fail_notices.load(SeqCst) {
            return Err(RoomsError("the fake Discord refuses every notice".into()));
        }
        Ok(())
    }
}

async fn voice_store() -> VoiceStore {
    let pool = attendance_pool().await;
    sqlx::raw_sql(VOICE_ROOMS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    VoiceStore::with_pool(pool).await.unwrap()
}

fn hub(channel: ChannelId, guild: GuildId, room_name: &str) -> Hub {
    Hub {
        channel,
        guild,
        room_name: room_name.to_owned(),
        created_by: ADMIN,
        created_at_ms: NOW,
    }
}

fn layout() -> HubLayout {
    HubLayout {
        category: Some(CATEGORY),
        position: 4,
        bitrate: Some(96_000),
        user_limit: Some(8),
        rtc_region: Some("rotterdam".to_owned()),
        video_quality: Some(2),
        overwrites: vec![
            Overwrite {
                target: OverwriteTarget::Role(EVERYONE),
                allow: 0,
                deny: Permissions::VIEW_CHANNEL.bits(),
            },
            Overwrite {
                target: OverwriteTarget::Role(CREWMATES),
                allow: (Permissions::VIEW_CHANNEL | Permissions::CONNECT).bits(),
                deny: 0,
            },
        ],
        blockers: Vec::new(),
    }
}

fn blocked_layout() -> HubLayout {
    HubLayout {
        blockers: BLOCKERS.to_vec(),
        ..layout()
    }
}

fn notice(user: UserId, failure: &Failure) -> String {
    format!(
        "<@{}> {}",
        user.get(),
        failure::failed("Barnacle could not open a room for you", failure)
    )
}

fn missing_permissions() -> DiscordRefusal {
    DiscordRefusal {
        status: 403,
        code: 50013,
        message: "Missing Permissions".to_owned(),
        fields: Vec::new(),
    }
}

fn only(events: Vec<serde_json::Value>) -> serde_json::Value {
    assert_eq!(events.len(), 1, "{events:?}");
    events.into_iter().next().unwrap()
}

async fn ready() -> (Arc<VoiceRooms<FakeRooms>>, FakeRooms, Hub) {
    let fake = FakeRooms::new();
    let voice = VoiceRooms::new(fake.clone(), voice_store().await);
    let cb = hub(HUB, GUILD, "cb");
    voice.store().add_hub(&cb).await.unwrap();
    (voice, fake, cb)
}

async fn open(voice: &VoiceRooms<FakeRooms>, hub: &Hub, user: UserId) -> ChannelId {
    match voice.joined(hub, user, &layout(), NOW).await {
        JoinOutcome::Opened { room, .. } => room,
        other => panic!("expected a room, got {other:?}"),
    }
}

fn all_empty(_: GuildId, _: ChannelId) -> Occupancy {
    Occupancy::Empty
}

fn all_occupied(_: GuildId, _: ChannelId) -> Occupancy {
    Occupancy::Occupied(1)
}

#[tokio::test]
async fn a_member_joining_a_hub_gets_a_numbered_room_and_is_moved_in() {
    let (voice, fake, cb) = ready().await;
    let outcome = voice.joined(&cb, AKI, &layout(), NOW).await;
    let room = ChannelId::new(FIRST_ROOM);
    assert_eq!(outcome, JoinOutcome::Opened { room, number: 1 });
    assert_eq!(fake.created().len(), 1);
    assert_eq!(fake.created()[0].name, "cb-1");
    assert_eq!(fake.moves(), vec![(AKI, room)]);
    assert!(fake.deleted().is_empty());
    assert_eq!(
        voice.store().room(room).await.unwrap(),
        Some(Room {
            channel: room,
            guild: GUILD,
            hub: HUB,
            room_name: "cb".to_owned(),
            number: 1,
            owner: AKI,
            created_at_ms: NOW,
            empty_since_ms: None,
        })
    );
    let second = voice.joined(&cb, BOREALIS, &layout(), NOW).await;
    assert_eq!(
        second,
        JoinOutcome::Opened {
            room: ChannelId::new(FIRST_ROOM + 1),
            number: 2
        }
    );
    assert_eq!(fake.created()[1].name, "cb-2");
}

async fn three_open_rooms() -> (Arc<VoiceRooms<FakeRooms>>, FakeRooms, Hub, [ChannelId; 3]) {
    let (voice, fake, cb) = ready().await;
    let rooms = [
        open(&voice, &cb, AKI).await,
        open(&voice, &cb, BOREALIS).await,
        open(&voice, &cb, ADMIN).await,
    ];
    let names: Vec<String> = fake.created().into_iter().map(|spec| spec.name).collect();
    assert_eq!(names, vec!["cb-1", "cb-2", "cb-3"]);
    (voice, fake, cb, rooms)
}

#[tokio::test]
async fn rooms_number_from_the_highest_open_room() {
    let (voice, fake, cb, [_, second, _]) = three_open_rooms().await;
    assert!(voice.store().remove_room(second).await.unwrap());
    assert_eq!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Opened {
            room: ChannelId::new(FIRST_ROOM + 3),
            number: 4
        }
    );
    assert_eq!(fake.created().last().unwrap().name, "cb-4");

    let (voice, fake, cb, [_, _, third]) = three_open_rooms().await;
    assert!(voice.store().remove_room(third).await.unwrap());
    assert_eq!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Opened {
            room: ChannelId::new(FIRST_ROOM + 3),
            number: 3
        }
    );
    assert_eq!(fake.created().last().unwrap().name, "cb-3");

    let (voice, fake, cb, rooms) = three_open_rooms().await;
    for room in rooms {
        assert!(voice.store().remove_room(room).await.unwrap());
    }
    assert_eq!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Opened {
            room: ChannelId::new(FIRST_ROOM + 3),
            number: 1
        }
    );
    assert_eq!(fake.created().last().unwrap().name, "cb-1");
}

#[tokio::test]
async fn four_members_joining_at_once_get_four_rooms() {
    let (voice, fake, cb) = ready().await;
    let joining: Vec<_> = [AKI, BOREALIS, ADMIN, UserId::new(400)]
        .into_iter()
        .map(|user| {
            let voice = Arc::clone(&voice);
            let cb = cb.clone();
            tokio::spawn(async move { voice.joined(&cb, user, &layout(), NOW).await })
        })
        .collect();
    let mut numbers = Vec::new();
    for join in joining {
        match join.await.unwrap() {
            JoinOutcome::Opened { number, .. } => numbers.push(number),
            other => panic!("expected a room, got {other:?}"),
        }
    }
    numbers.sort_unstable();
    assert_eq!(numbers, vec![1, 2, 3, 4]);
    let names: BTreeSet<String> = fake.created().into_iter().map(|spec| spec.name).collect();
    assert_eq!(
        names,
        BTreeSet::from(["cb-1", "cb-2", "cb-3", "cb-4"].map(str::to_owned))
    );
    assert_eq!(fake.moves().len(), 4);
    assert!(fake.deleted().is_empty());
    assert_eq!(voice.store().rooms().await.unwrap().len(), 4);
}

#[tokio::test]
async fn a_room_copies_the_hubs_category_permissions_and_settings() {
    let (voice, fake, cb) = ready().await;
    open(&voice, &cb, AKI).await;
    assert_eq!(
        fake.calls()[0],
        RoomsCall::Created {
            guild: GUILD,
            spec: RoomSpec {
                name: "cb-1".to_owned(),
                layout: layout(),
            },
        }
    );
}

#[tokio::test]
async fn an_empty_room_is_removed_after_five_minutes() {
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    let left_at = NOW + 1_000;
    voice.left(room, 0, left_at).await;
    assert_eq!(
        voice
            .store()
            .room(room)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        Some(left_at)
    );
    let kept = voice.sweep(&all_empty, left_at + 299_999).await;
    assert_eq!(kept, SweepReport::default());
    assert!(fake.deleted().is_empty());
    assert!(voice.store().room(room).await.unwrap().is_some());
    let closed = voice.sweep(&all_empty, left_at + 300_000).await;
    assert_eq!(
        closed,
        SweepReport {
            closed: 1,
            forgotten: 0,
            failures: 0
        }
    );
    assert_eq!(fake.deleted(), vec![room]);
    assert!(voice.store().room(room).await.unwrap().is_none());
    assert_eq!(voice.store().next_number(GUILD, "cb").await.unwrap(), 1);
}

#[tokio::test]
async fn a_member_returning_within_five_minutes_keeps_the_room() {
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.left(room, 0, NOW).await;
    voice.entered(room).await;
    assert_eq!(
        voice
            .store()
            .room(room)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        None
    );
    let report = voice.sweep(&all_occupied, NOW + 600_000).await;
    assert_eq!(report, SweepReport::default());
    assert!(fake.deleted().is_empty());
    assert!(voice.store().room(room).await.unwrap().is_some());
}

#[tokio::test]
async fn a_member_still_inside_keeps_the_room_unmarked() {
    let (voice, _, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.left(room, 1, NOW).await;
    assert_eq!(
        voice
            .store()
            .room(room)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        None
    );
}

#[tokio::test]
async fn a_member_who_left_voice_before_the_move_leaves_no_room_and_no_row() {
    let (voice, fake, cb) = ready().await;
    fake.answer_moves_not_in_voice(true);
    let outcome = voice.joined(&cb, AKI, &layout(), NOW).await;
    let room = ChannelId::new(FIRST_ROOM);
    assert_eq!(outcome, JoinOutcome::Abandoned);
    assert_eq!(fake.moves(), vec![(AKI, room)]);
    assert_eq!(fake.deleted(), vec![room]);
    assert!(voice.store().rooms().await.unwrap().is_empty());
    assert_eq!(voice.store().next_number(GUILD, "cb").await.unwrap(), 1);
}

#[tokio::test]
async fn a_refused_move_closes_the_room() {
    let (voice, fake, cb) = ready().await;
    fake.fail_moves(true);
    assert_eq!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Abandoned
    );
    assert_eq!(fake.deleted(), vec![ChannelId::new(FIRST_ROOM)]);
    assert!(voice.store().rooms().await.unwrap().is_empty());
}

#[tokio::test]
async fn an_abandoned_room_that_cannot_be_deleted_is_left_for_the_sweep() {
    let (voice, fake, cb) = ready().await;
    fake.answer_moves_not_in_voice(true);
    fake.fail_deletes(true);
    assert_eq!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Abandoned
    );
    let room = ChannelId::new(FIRST_ROOM);
    assert!(voice.store().room(room).await.unwrap().is_some());
    fake.fail_deletes(false);
    voice.sweep(&all_empty, NOW).await;
    let report = voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS).await;
    assert_eq!(report.closed, 1);
    assert!(voice.store().room(room).await.unwrap().is_none());
}

#[tokio::test]
async fn a_room_discord_refuses_leaves_the_member_where_they_are() {
    let (voice, fake, cb) = ready().await;
    fake.fail_creates(true);
    assert!(matches!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Failed(_)
    ));
    assert_eq!(fake.created().len(), 1);
    assert!(fake.moves().is_empty());
    assert!(fake.deleted().is_empty());
    assert!(voice.store().rooms().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_room_that_cannot_be_recorded_is_deleted_and_nobody_is_moved() {
    let (voice, fake, cb) = ready().await;
    let clash = Room {
        channel: ChannelId::new(FIRST_ROOM),
        guild: GUILD,
        hub: SECOND_HUB,
        room_name: "scrim".to_owned(),
        number: 1,
        owner: BOREALIS,
        created_at_ms: NOW,
        empty_since_ms: None,
    };
    voice.store().add_room(&clash).await.unwrap();
    assert!(matches!(
        voice.joined(&cb, AKI, &layout(), NOW).await,
        JoinOutcome::Failed(_)
    ));
    assert!(fake.moves().is_empty());
    assert_eq!(fake.deleted(), vec![ChannelId::new(FIRST_ROOM)]);
    assert_eq!(voice.store().rooms().await.unwrap(), vec![clash]);
}

#[tokio::test]
async fn hubs_sharing_a_room_name_never_open_two_rooms_with_one_name() {
    let (voice, fake, cb) = ready().await;
    let twin = hub(SECOND_HUB, GUILD, "cb");
    let elsewhere = hub(OTHER_HUB, OTHER_GUILD, "cb");
    voice.store().add_hub(&twin).await.unwrap();
    voice.store().add_hub(&elsewhere).await.unwrap();
    open(&voice, &cb, AKI).await;
    open(&voice, &twin, BOREALIS).await;
    open(&voice, &elsewhere, ADMIN).await;
    let names: Vec<String> = fake.created().into_iter().map(|spec| spec.name).collect();
    assert_eq!(names, vec!["cb-1", "cb-2", "cb-1"]);
    assert_eq!(voice.store().open_rooms_of(HUB).await.unwrap(), 1);
    assert_eq!(voice.store().open_rooms_of(SECOND_HUB).await.unwrap(), 1);
}

#[tokio::test]
async fn a_room_found_empty_by_a_sweep_is_marked_and_closes_five_minutes_later() {
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    let swept_at = NOW + 60_000;
    assert_eq!(
        voice.sweep(&all_empty, swept_at).await,
        SweepReport::default()
    );
    assert_eq!(
        voice
            .store()
            .room(room)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        Some(swept_at)
    );
    voice.sweep(&all_empty, swept_at + EMPTY_GRACE_MS - 1).await;
    assert!(fake.deleted().is_empty());
    let report = voice.sweep(&all_empty, swept_at + EMPTY_GRACE_MS).await;
    assert_eq!(report.closed, 1);
    assert_eq!(fake.deleted(), vec![room]);
    assert!(voice.store().room(room).await.unwrap().is_none());
}

#[tokio::test]
async fn a_sweep_that_finds_a_marked_room_occupied_clears_the_mark() {
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.left(room, 0, NOW).await;
    voice.sweep(&all_occupied, NOW + 1_000).await;
    assert_eq!(
        voice
            .store()
            .room(room)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        None
    );
    voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS).await;
    assert!(fake.deleted().is_empty());
}

#[tokio::test]
async fn a_room_that_cannot_be_deleted_is_counted_and_kept_for_the_next_sweep() {
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.left(room, 0, NOW).await;
    fake.fail_deletes(true);
    let failed = voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS).await;
    assert_eq!(
        failed,
        SweepReport {
            closed: 0,
            forgotten: 0,
            failures: 1
        }
    );
    assert!(voice.store().room(room).await.unwrap().is_some());
    fake.fail_deletes(false);
    let closed = voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS + 30_000).await;
    assert_eq!(closed.closed, 1);
    assert!(voice.store().room(room).await.unwrap().is_none());
}

#[tokio::test]
async fn a_sweep_forgets_rooms_and_hubs_reported_missing_and_leaves_unknown_ones_alone() {
    let (voice, fake, cb) = ready().await;
    let elsewhere = hub(OTHER_HUB, OTHER_GUILD, "cb");
    voice.store().add_hub(&elsewhere).await.unwrap();
    let gone = open(&voice, &cb, AKI).await;
    let kept = open(&voice, &elsewhere, BOREALIS).await;
    let missing_here = |guild: GuildId, _: ChannelId| {
        if guild == GUILD {
            Occupancy::Missing
        } else {
            Occupancy::Unknown
        }
    };
    let report = voice.sweep(&missing_here, NOW + NEW_CHANNEL_GRACE_MS).await;
    assert_eq!(
        report,
        SweepReport {
            closed: 0,
            forgotten: 1,
            failures: 0
        }
    );
    assert!(fake.deleted().is_empty());
    assert!(voice.store().room(gone).await.unwrap().is_none());
    assert!(voice.store().hub(HUB).await.unwrap().is_none());
    assert_eq!(
        voice
            .store()
            .room(kept)
            .await
            .unwrap()
            .unwrap()
            .empty_since_ms,
        None
    );
    assert_eq!(voice.store().hub(OTHER_HUB).await.unwrap(), Some(elsewhere));
}

#[tokio::test]
async fn a_channel_created_moments_ago_is_kept_though_the_cache_does_not_show_it_yet() {
    let (voice, _, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    let missing = |_: GuildId, _: ChannelId| Occupancy::Missing;
    let report = voice.sweep(&missing, NOW + NEW_CHANNEL_GRACE_MS - 1).await;
    assert_eq!(report, SweepReport::default());
    assert!(voice.store().room(room).await.unwrap().is_some());
    assert!(voice.store().hub(HUB).await.unwrap().is_some());
}

#[tokio::test]
async fn a_deleted_channel_is_forgotten_whether_hub_or_room() {
    let (voice, _, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.channel_deleted(room).await;
    assert!(voice.store().room(room).await.unwrap().is_none());
    assert!(voice.store().hub(HUB).await.unwrap().is_some());
    let other = open(&voice, &cb, BOREALIS).await;
    voice.channel_deleted(HUB).await;
    assert!(voice.store().hub(HUB).await.unwrap().is_none());
    assert!(voice.store().room(other).await.unwrap().is_some());
    voice.channel_deleted(CATEGORY).await;
    assert_eq!(voice.store().rooms().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_join_barnacle_cannot_copy_is_refused_before_calling_discord() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    let failure = match voice.joined(&cb, AKI, &blocked_layout(), NOW).await {
        JoinOutcome::Refused(failure) => failure,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(failure.kind, Kind::MissingPermissions);
    assert_eq!(failure.missing, BLOCKERS.to_vec());
    assert!(fake.created().is_empty());
    assert!(fake.moves().is_empty());
    assert!(voice.store().rooms().await.unwrap().is_empty());
    let notices = fake.notices();
    assert_eq!(notices, vec![(HUB, AKI, notice(AKI, &failure))]);
    let (_, _, content) = &notices[0];
    assert!(content.starts_with("<@200> Barnacle could not open a room for you."));
    assert!(content.contains("Manage Channels and Move Members"));
    let event = only(logs.named("voice.room.refused"));
    assert_eq!(event["level"], "WARN");
    assert_eq!(event["error.type"], "discord.missing_permissions");
    assert_eq!(event["barnacle.reference"], failure.reference.as_str());
    assert_eq!(
        event["barnacle.permissions.missing"],
        "Manage Channels, Move Members"
    );
    assert_eq!(event["barnacle.hub.id"], HUB.get().to_string());
    assert_eq!(event["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(event["discord.user.id"], AKI.get().to_string());
}

#[tokio::test]
async fn a_member_is_told_once_a_minute_why_rooms_cannot_open() {
    let (voice, fake, cb) = ready().await;
    let blocked = blocked_layout();
    voice.joined(&cb, AKI, &blocked, NOW).await;
    voice.joined(&cb, BOREALIS, &blocked, NOW + 10_000).await;
    assert_eq!(fake.notices().len(), 1);
    assert_eq!(fake.notices()[0].1, AKI);
    voice
        .joined(&cb, BOREALIS, &blocked, NOW + NOTICE_INTERVAL_MS - 1)
        .await;
    assert_eq!(fake.notices().len(), 1);
    voice.joined(&cb, BOREALIS, &blocked, NOW + 61_000).await;
    let notices = fake.notices();
    assert_eq!(notices.len(), 2);
    assert_eq!((notices[1].0, notices[1].1), (HUB, BOREALIS));
    assert!(fake.created().is_empty());
}

#[tokio::test]
async fn each_hub_has_its_own_notice_minute() {
    let (voice, fake, cb) = ready().await;
    let twin = hub(SECOND_HUB, GUILD, "scrim");
    voice.store().add_hub(&twin).await.unwrap();
    voice.joined(&cb, AKI, &blocked_layout(), NOW).await;
    voice
        .joined(&twin, BOREALIS, &blocked_layout(), NOW + 1_000)
        .await;
    let hubs: Vec<ChannelId> = fake.notices().into_iter().map(|(hub, _, _)| hub).collect();
    assert_eq!(hubs, vec![HUB, SECOND_HUB]);
}

#[tokio::test]
async fn a_room_discord_refuses_is_reported_with_its_reason() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    fake.refuse_creates(missing_permissions());
    let failure = match voice.joined(&cb, AKI, &layout(), NOW).await {
        JoinOutcome::Failed(failure) => failure,
        other => panic!("expected a failure, got {other:?}"),
    };
    assert_eq!(failure.kind, Kind::MissingPermissions);
    let event = only(logs.named("voice.room.open_failed"));
    assert_eq!(event["level"], "WARN");
    assert_eq!(event["event.outcome"], "failure");
    assert_eq!(event["error.type"], "discord.missing_permissions");
    assert_eq!(event["barnacle.reference"], failure.reference.as_str());
    assert_eq!(event["barnacle.hub.id"], HUB.get().to_string());
    assert_eq!(event["discord.error.code"], 50013);
    assert_eq!(event["http.response.status_code"], 403);
    assert_eq!(event["barnacle.room.number"], 1);
    assert_eq!(fake.notices(), vec![(HUB, AKI, notice(AKI, &failure))]);
    assert!(fake.moves().is_empty());
}

#[tokio::test]
async fn a_room_that_cannot_be_recorded_is_reported_and_the_member_told() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    let clash = Room {
        channel: ChannelId::new(FIRST_ROOM),
        guild: GUILD,
        hub: SECOND_HUB,
        room_name: "scrim".to_owned(),
        number: 1,
        owner: BOREALIS,
        created_at_ms: NOW,
        empty_since_ms: None,
    };
    voice.store().add_room(&clash).await.unwrap();
    let failure = match voice.joined(&cb, AKI, &layout(), NOW).await {
        JoinOutcome::Failed(failure) => failure,
        other => panic!("expected a failure, got {other:?}"),
    };
    assert_eq!(failure.kind, Kind::Database);
    let event = only(logs.named("voice.room.open_failed"));
    assert_eq!(event["level"], "ERROR");
    assert_eq!(event["barnacle.reference"], failure.reference.as_str());
    assert_eq!(event["barnacle.room.id"], FIRST_ROOM.to_string());
    assert_eq!(fake.notices(), vec![(HUB, AKI, notice(AKI, &failure))]);
}

#[tokio::test]
async fn a_notice_that_cannot_be_sent_is_reported() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    fake.fail_notices(true);
    let outcome = voice.joined(&cb, AKI, &blocked_layout(), NOW).await;
    assert!(matches!(outcome, JoinOutcome::Refused(_)));
    let event = only(logs.named("voice.notice.failed"));
    assert_eq!(event["event.outcome"], "failure");
    assert!(
        event["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake Discord refuses every notice")
    );
    assert_eq!(event["barnacle.hub.id"], HUB.get().to_string());
}

#[tokio::test]
async fn an_opened_room_is_recorded_with_its_hub_room_and_number() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    let event = only(logs.named("voice.room.opened"));
    assert_eq!(event["level"], "INFO");
    assert_eq!(event["event.outcome"], "success");
    assert_eq!(event["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(event["discord.user.id"], AKI.get().to_string());
    assert_eq!(event["barnacle.hub.id"], HUB.get().to_string());
    assert_eq!(event["barnacle.room.id"], room.get().to_string());
    assert_eq!(event["barnacle.room.number"], 1);
    assert!(fake.notices().is_empty());
}

#[tokio::test]
async fn an_abandoned_room_is_recorded_and_a_refused_move_carries_its_reason() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    fake.answer_moves_not_in_voice(true);
    voice.joined(&cb, AKI, &layout(), NOW).await;
    fake.answer_moves_not_in_voice(false);
    fake.fail_moves(true);
    voice.joined(&cb, BOREALIS, &layout(), NOW).await;
    let abandoned = logs.named("voice.room.abandoned");
    assert_eq!(abandoned.len(), 2, "{abandoned:?}");
    assert_eq!(abandoned[0]["event.outcome"], "success");
    assert_eq!(abandoned[0]["discord.user.id"], AKI.get().to_string());
    assert_eq!(abandoned[1]["event.outcome"], "failure");
    assert_eq!(abandoned[1]["discord.user.id"], BOREALIS.get().to_string());
    assert!(
        abandoned[1]["exception.message"]
            .as_str()
            .unwrap()
            .contains("the fake Discord refuses every move")
    );
    assert!(logs.named("voice.room.opened").is_empty());
}

#[tokio::test]
async fn a_room_the_sweep_cannot_delete_is_reported_with_its_room() {
    let logs = common::logs::capture();
    let (voice, fake, cb) = ready().await;
    let room = open(&voice, &cb, AKI).await;
    voice.left(room, 0, NOW).await;
    fake.fail_deletes(true);
    voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS).await;
    let event = only(logs.named("voice.room.close_failed"));
    assert_eq!(event["level"], "ERROR");
    assert_eq!(event["discord.guild.id"], GUILD.get().to_string());
    assert_eq!(event["barnacle.hub.id"], HUB.get().to_string());
    assert_eq!(event["barnacle.room.id"], room.get().to_string());
    assert_eq!(event["barnacle.room.number"], 1);
    assert!(logs.named("voice.room.closed").is_empty());
    fake.fail_deletes(false);
    voice.sweep(&all_empty, NOW + EMPTY_GRACE_MS + 30_000).await;
    let closed = only(logs.named("voice.room.closed"));
    assert_eq!(closed["barnacle.room.id"], room.get().to_string());
}

#[test]
fn clean_name_trims_and_enforces_both_limits() {
    assert_eq!(clean_name("  cb \n", ROOM_NAME_LIMIT), Ok("cb".to_owned()));
    assert_eq!(clean_name(" \t ", ROOM_NAME_LIMIT), Err(NameProblem::Empty));
    assert_eq!(clean_name("", HUB_NAME_LIMIT), Err(NameProblem::Empty));
    let longest_room = "r".repeat(90);
    assert_eq!(
        clean_name(&format!(" {longest_room} "), ROOM_NAME_LIMIT),
        Ok(longest_room.clone())
    );
    assert_eq!(
        clean_name(&format!("{longest_room}r"), ROOM_NAME_LIMIT),
        Err(NameProblem::TooLong { limit: 90 })
    );
    let longest_hub = "\u{e9}".repeat(100);
    assert_eq!(
        clean_name(&longest_hub, HUB_NAME_LIMIT),
        Ok(longest_hub.clone())
    );
    assert_eq!(
        clean_name(&format!("{longest_hub}h"), HUB_NAME_LIMIT),
        Err(NameProblem::TooLong { limit: 100 })
    );
}

#[test]
fn a_room_is_named_after_its_room_name_and_number() {
    assert_eq!(room_channel_name("cb", 1), "cb-1");
    assert_eq!(room_channel_name("Scrim Room", 12), "Scrim Room-12");
}

#[test]
fn room_access_is_view_connect_move_and_manage() {
    assert_eq!(
        Permissions::from_bits_truncate(ROOM_ACCESS),
        Permissions::VIEW_CHANNEL
            | Permissions::CONNECT
            | Permissions::MOVE_MEMBERS
            | Permissions::MANAGE_CHANNELS
    );
}

#[test]
fn with_room_access_adds_barnacles_overwrite_and_merges_it_into_an_existing_one() {
    let copied = layout().overwrites;
    let added = with_room_access(&copied, BARNACLE);
    assert_eq!(added.len(), 3);
    assert_eq!(&added[..2], &copied[..]);
    assert_eq!(
        added[2],
        Overwrite {
            target: OverwriteTarget::Member(BARNACLE),
            allow: ROOM_ACCESS,
            deny: 0,
        }
    );
    let speak = Permissions::SPEAK.bits();
    let own = Overwrite {
        target: OverwriteTarget::Member(BARNACLE),
        allow: speak,
        deny: Permissions::CONNECT.bits() | Permissions::STREAM.bits(),
    };
    let someone = Overwrite {
        target: OverwriteTarget::Member(AKI),
        allow: 0,
        deny: Permissions::CONNECT.bits(),
    };
    let merged = with_room_access(&[copied[0], own, someone], BARNACLE);
    assert_eq!(
        merged,
        vec![
            copied[0],
            Overwrite {
                target: OverwriteTarget::Member(BARNACLE),
                allow: speak | ROOM_ACCESS,
                deny: Permissions::STREAM.bits(),
            },
            someone,
        ]
    );
}

#[test]
fn hub_replies_read_as_written() {
    assert_eq!(
        voice_text::hub_created(HUB, "cb"),
        "Created <#10>. Joining it opens a room named `cb-1`, `cb-2` and so on, and moves the member in. Set who can see and join it in its channel permissions; every room copies them."
    );
    assert_eq!(
        voice_text::hub_edited(
            HUB,
            &[
                voice_text::renamed("Scrims"),
                voice_text::rooms_renamed("scrim"),
                voice_text::moved_to(CATEGORY),
            ]
        ),
        "Updated <#10>: renamed to Scrims; new rooms are named `scrim-1`, `scrim-2` and so on; moved to <#30>. Rooms already open keep their names."
    );
    assert_eq!(
        voice_text::hub_edited(HUB, &[voice_text::MOVED_TO_TOP.to_owned()]),
        "Updated <#10>: moved out of its category. Rooms already open keep their names."
    );
    assert_eq!(
        voice_text::hub_removed("Join to Create"),
        "Deleted the Join to Create channel Join to Create. Rooms it opened stay until they empty, then close as usual."
    );
    assert_eq!(
        voice_text::name_too_long(90),
        "That name is too long; the limit is 90 characters."
    );
    assert_eq!(
        voice_text::hub_list(&[
            voice_text::hub_line(HUB, "cb", 2),
            voice_text::hub_line(SECOND_HUB, "scrim", 0),
        ]),
        "Join to Create channels in this server:\n- <#10> opens `cb-#` rooms, 2 open now\n- <#11> opens `scrim-#` rooms, 0 open now"
    );
}

use crate::config::{Config, Credentials};
use crate::error::{BridgeError, Result};
use crate::trust::apply_trust_policy;
use matrix_sdk::{
    config::SyncSettings,
    ruma::{
        events::room::message::RoomMessageEventContent,
        OwnedRoomId,
    },
    Client, Room,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// A message returned from read operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub sender: String,
    pub body: String,
    pub timestamp: String,
    pub event_id: String,
    pub decryption_failed: bool,
}

/// A room returned from list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub room_id: String,
    pub name: Option<String>,
    pub encrypted: bool,
    pub member_count: u64,
}

/// Core Matrix E2EE client wrapper.
pub struct MatrixBridgeClient {
    client: Client,
    config: Config,
    has_synced: bool,
    sync_task: Option<JoinHandle<()>>,
}

impl MatrixBridgeClient {
    /// Build a client and login with password. Creates a new device and crypto store.
    pub async fn login_with_password(
        config: &Config,
        password: &str,
    ) -> Result<Self> {
        config.ensure_store_dir()?;

        let client = Client::builder()
            .homeserver_url(&config.homeserver)
            .sqlite_store(&config.store_path, None)
            .build()
            .await
            .map_err(|e| BridgeError::Matrix(e.to_string()))?;

        info!("logging in as {}...", config.user_id);
        let login = client
            .matrix_auth()
            .login_username(&config.user_id, password)
            .initial_device_display_name(&config.device_name)
            .await
            .map_err(|e| BridgeError::LoginFailed(e.to_string()))?;

        info!("logged in, device_id={}", login.device_id);

        let creds = Credentials {
            access_token: login.access_token,
            user_id: login.user_id.to_string(),
            device_id: login.device_id.to_string(),
        };
        creds.save(config)?;

        info!("running initial sync...");
        client
            .sync_once(SyncSettings::default())
            .await
            .map_err(|e| BridgeError::SyncFailed(e.to_string()))?;

        apply_trust_policy(&client, &config.trust_mode).await;

        Ok(Self {
            client,
            config: config.clone(),
            has_synced: true,
            sync_task: None,
        })
    }

    /// Restore a previously saved session.
    pub async fn restore(config: &Config) -> Result<Self> {
        config.ensure_store_dir()?;
        let creds = Credentials::load(config)?;

        let client = Client::builder()
            .homeserver_url(&config.homeserver)
            .sqlite_store(&config.store_path, None)
            .build()
            .await
            .map_err(|e| BridgeError::Matrix(e.to_string()))?;

        let session = matrix_sdk::authentication::matrix::MatrixSession {
            meta: matrix_sdk::SessionMeta {
                user_id: creds
                    .user_id
                    .as_str()
                    .try_into()
                    .map_err(|e: matrix_sdk::IdParseError| BridgeError::Config(e.to_string()))?,
                device_id: creds
                    .device_id
                    .as_str()
                    .into(),
            },
            tokens: matrix_sdk::SessionTokens {
                access_token: creds.access_token,
                refresh_token: None,
            },
        };

        client
            .restore_session(session)
            .await
            .map_err(|e| BridgeError::Matrix(e.to_string()))?;

        debug!("session restored for {}", config.user_id);

        Ok(Self {
            client,
            config: config.clone(),
            has_synced: false,
            sync_task: None,
        })
    }

    /// Run a single sync cycle. First call does full state.
    pub async fn sync_once(&mut self) -> Result<()> {
        let settings = if self.has_synced {
            SyncSettings::default().timeout(Duration::from_secs(10))
        } else {
            SyncSettings::default()
        };

        self.client
            .sync_once(settings)
            .await
            .map_err(|e| BridgeError::SyncFailed(e.to_string()))?;

        if !self.has_synced {
            apply_trust_policy(&self.client, &self.config.trust_mode).await;
            self.has_synced = true;
        }

        Ok(())
    }

    /// Start a background sync loop (for MCP server).
    pub async fn start_sync(&mut self) -> Result<()> {
        self.sync_once().await?;

        let client = self.client.clone();
        let trust_mode = self.config.trust_mode.clone();

        let handle = tokio::spawn(async move {
            loop {
                let settings = SyncSettings::default().timeout(Duration::from_secs(30));
                match client.sync_once(settings).await {
                    Ok(_) => {
                        apply_trust_policy(&client, &trust_mode).await;
                    }
                    Err(e) => {
                        warn!("sync error: {}, retrying in 5s", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        self.sync_task = Some(handle);
        Ok(())
    }

    /// Send a message to a room, optionally with an @mention.
    pub async fn send_message(
        &self,
        room_id: &str,
        body: &str,
        mention: Option<&str>,
    ) -> Result<String> {
        let room = self.get_room(room_id)?;

        let content = if let Some(mention_user) = mention {
            let display = mention_user
                .strip_prefix('@')
                .and_then(|s| s.split(':').next())
                .unwrap_or(mention_user);
            let html = format!(
                "<a href=\"https://matrix.to/#/{}\">{}</a> {}",
                mention_user, display, body
            );
            let plain = format!("{} {}", display, body);
            RoomMessageEventContent::text_html(plain, html)
        } else {
            RoomMessageEventContent::text_plain(body)
        };

        let response = room
            .send(content)
            .await
            .map_err(|e| BridgeError::SendFailed(e.to_string()))?;

        Ok(response.event_id.to_string())
    }

    /// Read recent messages from a room.
    /// Uses room.messages() which auto-decrypts E2EE events via the SDK's olm machine.
    pub async fn read_messages(
        &self,
        room_id: &str,
        limit: u32,
    ) -> Result<Vec<Message>> {
        use matrix_sdk::deserialized_responses::TimelineEventKind;
        use matrix_sdk::room::MessagesOptions;
        use matrix_sdk::ruma::events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent};

        let room = self.get_room(room_id)?;

        let mut options = MessagesOptions::backward();
        options.limit = limit.into();

        let response = room
            .messages(options)
            .await
            .map_err(|e| BridgeError::Matrix(e.to_string()))?;

        let mut messages = Vec::new();

        for timeline_event in &response.chunk {
            let decryption_failed = matches!(&timeline_event.kind, TimelineEventKind::UnableToDecrypt { .. });

            // .raw() returns the decrypted event for Decrypted, or the raw event for PlainText/UTD
            match timeline_event.kind.raw().deserialize() {
                Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(msg))) => {
                    let body = match msg.as_original() {
                        Some(original) => original.content.body().to_string(),
                        None => continue,
                    };

                    let ts = msg.origin_server_ts();
                    let millis = i64::from(ts.0);
                    let timestamp = chrono::DateTime::from_timestamp_millis(millis)
                        .map(|dt| dt.format("%H:%M:%S").to_string())
                        .unwrap_or_default();

                    messages.push(Message {
                        sender: msg.sender().to_string(),
                        body,
                        timestamp,
                        event_id: msg.event_id().to_string(),
                        decryption_failed: false,
                    });
                }
                Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomEncrypted(enc))) => {
                    let ts = enc.origin_server_ts();
                    let millis = i64::from(ts.0);
                    let timestamp = chrono::DateTime::from_timestamp_millis(millis)
                        .map(|dt| dt.format("%H:%M:%S").to_string())
                        .unwrap_or_default();

                    messages.push(Message {
                        sender: enc.sender().to_string(),
                        body: "[encrypted message — unable to decrypt]".to_string(),
                        timestamp,
                        event_id: enc.event_id().to_string(),
                        decryption_failed: true,
                    });
                }
                Err(e) => {
                    if decryption_failed {
                        messages.push(Message {
                            sender: "unknown".to_string(),
                            body: "[encrypted message — unable to decrypt]".to_string(),
                            timestamp: String::new(),
                            event_id: String::new(),
                            decryption_failed: true,
                        });
                    } else {
                        debug!("failed to deserialize event: {}", e);
                    }
                }
                _ => {} // skip non-message events
            }
        }

        messages.reverse(); // chronological order
        Ok(messages)
    }

    /// List joined rooms.
    pub async fn get_rooms(&self) -> Vec<RoomInfo> {
        let mut rooms = Vec::new();
        for room in self.client.rooms() {
            rooms.push(RoomInfo {
                room_id: room.room_id().to_string(),
                name: room.name(),
                encrypted: room.latest_encryption_state().await.map(|s| s.is_encrypted()).unwrap_or(false),
                member_count: room.joined_members_count(),
            });
        }
        rooms
    }

    /// Join a room by ID or alias.
    pub async fn join_room(&self, room_id_or_alias: &str) -> Result<String> {
        let room = if room_id_or_alias.starts_with('!') {
            let id: OwnedRoomId = room_id_or_alias
                .try_into()
                .map_err(|_| BridgeError::RoomNotFound(room_id_or_alias.to_string()))?;
            self.client
                .join_room_by_id(&id)
                .await
                .map_err(|e| BridgeError::Matrix(e.to_string()))?
        } else {
            let alias = matrix_sdk::ruma::RoomOrAliasId::parse(room_id_or_alias)
                .map_err(|_| BridgeError::RoomNotFound(room_id_or_alias.to_string()))?;
            self.client
                .join_room_by_id_or_alias(&alias, &[])
                .await
                .map_err(|e| BridgeError::Matrix(e.to_string()))?
        };

        Ok(room.room_id().to_string())
    }

    /// Send a message and wait for a reply from another user.
    /// Polls with increasing window to handle chatty rooms.
    pub async fn send_and_wait(
        &self,
        room_id: &str,
        body: &str,
        mention: Option<&str>,
        timeout_secs: u64,
    ) -> Result<Option<Message>> {
        let event_id = self.send_message(room_id, body, mention).await?;
        let own_user = self.client.user_id().map(|u| u.to_string()).unwrap_or_default();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
        let mut poll_window: u32 = 20;

        while tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let msgs = self.read_messages(room_id, poll_window.min(100)).await?;
            let mut found_ours = false;
            for msg in &msgs {
                if msg.event_id == event_id {
                    found_ours = true;
                    continue;
                }
                if found_ours && msg.sender != own_user && !msg.decryption_failed {
                    return Ok(Some(msg.clone()));
                }
            }

            // Widen the window if our message is scrolling out
            if !found_ours && poll_window < 100 {
                poll_window = (poll_window + 20).min(100);
            }
        }

        Ok(None)
    }

    /// Get the inner matrix-sdk Client (for MCP event handlers).
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Get the config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    fn get_room(&self, room_id: &str) -> Result<Room> {
        let id: OwnedRoomId = room_id
            .try_into()
            .map_err(|_| BridgeError::RoomNotFound(room_id.to_string()))?;
        self.client
            .get_room(&id)
            .ok_or_else(|| BridgeError::RoomNotFound(room_id.to_string()))
    }
}

impl Drop for MatrixBridgeClient {
    fn drop(&mut self) {
        if let Some(handle) = self.sync_task.take() {
            handle.abort();
        }
    }
}

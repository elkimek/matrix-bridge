use crate::client::MatrixBridgeClient;
use crate::error::Result;
use rmcp::{
    ServerHandler, ServiceExt, tool, tool_router,
    handler::server::router::{Router, tool::ToolRouter},
    handler::server::wrapper::{Json, Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct BridgeServer {
    client: Arc<RwLock<MatrixBridgeClient>>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

// --- Tool parameter/output types ---

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendMessageParams {
    /// The Matrix room ID (e.g. !abc123:matrix.org)
    pub room_id: String,
    /// The message text to send
    pub message: String,
    /// Optional user ID to @mention (e.g. @user:matrix.org)
    #[serde(default)]
    pub mention: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SendMessageOutput {
    pub event_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendAndWaitParams {
    /// The Matrix room ID
    pub room_id: String,
    /// The message text to send
    pub message: String,
    /// Optional user ID to @mention
    #[serde(default)]
    pub mention: Option<String>,
    /// Timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SendAndWaitOutput {
    pub reply: Option<MessageOutput>,
    pub timed_out: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MessageOutput {
    pub sender: String,
    pub body: String,
    pub timestamp: String,
    pub event_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReadMessagesParams {
    /// The Matrix room ID
    pub room_id: String,
    /// Number of messages to read (default: 20)
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReadMessagesOutput {
    pub messages: Vec<MessageOutput>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ListRoomsOutput {
    pub rooms: Vec<RoomOutput>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct RoomOutput {
    pub room_id: String,
    pub name: Option<String>,
    pub encrypted: bool,
    pub member_count: u64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct JoinRoomParams {
    /// Room ID or alias to join
    pub room_id: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct JoinRoomOutput {
    pub room_id: String,
}

// --- Tool implementations ---

#[tool_router]
impl BridgeServer {
    #[tool(
        name = "send_message",
        description = "Send a message to a Matrix room. Automatically encrypted if the room has E2EE enabled."
    )]
    async fn send_message(
        &self,
        Parameters(params): Parameters<SendMessageParams>,
    ) -> Json<SendMessageOutput> {
        let client = self.client.read().await;
        let event_id = client
            .send_message(&params.room_id, &params.message, params.mention.as_deref())
            .await
            .unwrap_or_else(|e| format!("error: {}", e));
        Json(SendMessageOutput { event_id })
    }

    #[tool(
        name = "send_and_wait",
        description = "Send a message and wait for a reply from another user. Returns the first reply or times out."
    )]
    async fn send_and_wait(
        &self,
        Parameters(params): Parameters<SendAndWaitParams>,
    ) -> Json<SendAndWaitOutput> {
        let client = self.client.read().await;
        let timeout = params.timeout.clamp(1, 300);
        match client
            .send_and_wait(
                &params.room_id,
                &params.message,
                params.mention.as_deref(),
                timeout,
            )
            .await
        {
            Ok(Some(msg)) => Json(SendAndWaitOutput {
                reply: Some(MessageOutput {
                    sender: msg.sender,
                    body: msg.body,
                    timestamp: msg.timestamp,
                    event_id: msg.event_id,
                }),
                timed_out: false,
            }),
            _ => Json(SendAndWaitOutput {
                reply: None,
                timed_out: true,
            }),
        }
    }

    #[tool(
        name = "read_messages",
        description = "Read recent messages from a Matrix room, decrypting E2EE messages automatically."
    )]
    async fn read_messages(
        &self,
        Parameters(params): Parameters<ReadMessagesParams>,
    ) -> Json<ReadMessagesOutput> {
        let client = self.client.read().await;
        let limit = params.limit.clamp(1, 100);
        let messages = client
            .read_messages(&params.room_id, limit)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|m| !m.decryption_failed)
            .map(|m| MessageOutput {
                sender: m.sender,
                body: m.body,
                timestamp: m.timestamp,
                event_id: m.event_id,
            })
            .collect();
        Json(ReadMessagesOutput { messages })
    }

    #[tool(
        name = "list_rooms",
        description = "List all Matrix rooms the bridge has joined."
    )]
    async fn list_rooms(&self) -> Json<ListRoomsOutput> {
        let client = self.client.read().await;
        let rooms = client
            .get_rooms()
            .await
            .into_iter()
            .map(|r| RoomOutput {
                room_id: r.room_id,
                name: r.name,
                encrypted: r.encrypted,
                member_count: r.member_count,
            })
            .collect();
        Json(ListRoomsOutput { rooms })
    }

    #[tool(
        name = "join_room",
        description = "Join a Matrix room by room ID or alias."
    )]
    async fn join_room(
        &self,
        Parameters(params): Parameters<JoinRoomParams>,
    ) -> Json<JoinRoomOutput> {
        let client = self.client.read().await;
        let room_id = client
            .join_room(&params.room_id)
            .await
            .unwrap_or_else(|e| format!("error: {}", e));
        Json(JoinRoomOutput { room_id })
    }
}

impl ServerHandler for BridgeServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_instructions("E2EE Matrix bridge. Send and read encrypted messages in Matrix rooms. Messages are automatically encrypted/decrypted.")
    }
}

impl BridgeServer {
    pub fn new(client: Arc<RwLock<MatrixBridgeClient>>) -> Router<Self> {
        let server = Self {
            client,
            tool_router: Self::tool_router(),
        };
        Router::new(server)
    }
}

/// Start the MCP server on stdio.
pub async fn serve(client: Arc<RwLock<MatrixBridgeClient>>) -> Result<()> {
    use rmcp::transport::io::stdio;

    let router = BridgeServer::new(client);
    let transport = stdio();

    router
        .serve(transport)
        .await
        .map_err(|e| crate::error::BridgeError::Other(format!("MCP server error: {}", e)))?
        .waiting()
        .await
        .map_err(|e| crate::error::BridgeError::Other(format!("MCP server error: {}", e)))?;

    Ok(())
}

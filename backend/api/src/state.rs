use crate::auth::service::AuthService;
use crate::config::AppConfig;
use crate::conversations::repo::ConversationRepo;
use crate::files::repo::FileRepo;
use crate::files::storage::FileStorage;
use crate::hooks::repo::HookRepo;
use crate::huddle::repo::HuddleRepo;
use crate::messaging::publisher::EventPublisher;
use crate::messaging::repo::MessageRepo;
use crate::notifications::repo::NotificationRepo;
use crate::saved::repo::SavedRepo;
use crate::scheduled::repo::ScheduledRepo;
use crate::workspace::service::WorkspaceService;

pub struct AppState {
    pub config: AppConfig,
    pub pool: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub auth_service: AuthService,
    pub workspace_service: WorkspaceService,
    pub message_repo: MessageRepo,
    pub publisher: EventPublisher,
    pub file_repo: FileRepo,
    pub file_storage: Box<dyn FileStorage>,
    pub hook_repo: HookRepo,
    pub notification_repo: NotificationRepo,
    pub conversation_repo: ConversationRepo,
    pub saved_repo: SavedRepo,
    pub scheduled_repo: ScheduledRepo,
    pub retention_repo: crate::retention::repo::RetentionRepo,
    pub export_repo: crate::export::repo::ExportRepo,
    pub totp_repo: crate::auth::totp_repo::TotpRepo,
    pub scim_repo: crate::scim::repo::ScimRepo,
    pub push_repo: crate::push::repo::PushRepo,
    pub push_sender: crate::push::sender::PushSender,
    pub emoji_repo: crate::emoji::repo::EmojiRepo,
    pub group_repo: crate::groups::repo::GroupRepo,
    pub huddle_repo: HuddleRepo,
}

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
    pub scheduled_repo: ScheduledRepo,
    pub huddle_repo: HuddleRepo,
}

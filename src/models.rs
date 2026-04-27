use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use tera::Tera;
use std::sync::LazyLock;

#[derive(Clone, Debug, Serialize)]
pub struct SseEvent {
  pub target_uid: i64,
  pub event_type: String,
  pub message: String,
}

#[derive(Clone)]
pub struct AppState {
  pub db_pool: MySqlPool,
  pub github_client_id: String,
  pub github_client_secret: String,
  pub sse_sender: tokio::sync::broadcast::Sender<SseEvent>,
  pub redis_pool: redis::aio::ConnectionManager,
}

#[derive(sqlx::FromRow)]
pub struct PostRow {
  pub ib_uid: String,
  pub username: String,
  pub postid: String,
  pub post: String,
  pub timestamp: String,
  pub acknowledged_count: i64,
  pub user_total_acks: i64,
  pub pinned_postid: Option<String>,
}

#[derive(sqlx::FromRow, Default)]
pub struct ProRow {
  pub ibp: String,
  pub pro: String,
  pub location: String,
  pub services: String,
  pub website: String,
  // github: String,
}

#[derive(sqlx::FromRow)]
pub struct ProfileLookupRow {
  pub ib_uid: String,
}

#[derive(sqlx::FromRow)]
pub struct UsernameLookupRow {
  pub ib_uid: String,
}

#[derive(sqlx::FromRow)]
pub struct RelatedUsernameRankRow {
  pub username: String,
  pub total_acknowledgments: i64,
}

#[derive(sqlx::FromRow)]
pub struct RelatedCandidateRow {
  pub username: String,
  pub total_acknowledgments: i64,
  pub github: String,
  pub ibp: String,
  pub pro: String,
  pub services: String,
  pub location: String,
  pub website: String,
}

#[derive(sqlx::FromRow)]
pub struct SearchUserRow {
  pub username: String,
  pub total_acknowledgments: i64,
  pub ibp: String,
}

#[derive(sqlx::FromRow)]
pub struct TrendingTagRow {
  pub tag: String,
  pub tag_count: i64,
}

#[derive(sqlx::FromRow)]
pub struct RecentPostTagBackfillRow {
  pub postid: String,
  pub post: String,
}

#[derive(sqlx::FromRow)]
pub struct AdvertImageRow {
  pub imageid: i64,
  pub imagepath: String,
  pub url: String,
}

#[derive(sqlx::FromRow)]
pub struct AdvertImageAdminRow {
  pub imageid: i64,
  pub imagepath: String,
  pub url: String,
  pub clicks: i64,
  pub views: i64,
}

#[derive(sqlx::FromRow)]
pub struct AdvertOwnedRow {
  pub imageid: i64,
  pub imagepath: String,
  pub url: String,
  pub clicks: i64,
  pub views: i64,
  pub payment_status: String,
}

#[derive(Deserialize)]
pub struct GithubCallback {
  pub code: String,
  pub state: String,
}

#[derive(Deserialize)]
pub struct GithubTokenResponse {
  pub access_token: String,
}

#[derive(Deserialize)]
pub struct GithubTokenErrorResponse {
  pub error: String,
  pub error_description: Option<String>,
}

#[derive(Deserialize)]
pub struct GithubUser {
  pub login: String,
  pub id: u64,
}

#[derive(Deserialize)]
pub struct NewPostRequest {
  pub post: String,
}

#[derive(Deserialize)]
pub struct DeletePostRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
  pub root_pid: Option<String>,
  pub post_owner_uid: Option<i64>,
}

#[derive(Deserialize)]
pub struct EditPostRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
  pub root_pid: Option<String>,
  pub post_owner_uid: Option<i64>,
}

#[derive(Deserialize)]
pub struct EditPostUpdateRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
  pub post: String,
  pub root_pid: Option<String>,
  pub post_owner_uid: Option<i64>,
}

#[derive(Deserialize)]
pub struct ShowPostRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
}

#[derive(Deserialize)]
pub struct PinPostRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
}

#[derive(Deserialize)]
pub struct ReplyRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
  pub post: String,
}

#[derive(Deserialize)]
pub struct EditProfileRequest {
  pub ib_uid: i64,
  pub ib_user: String,
}

#[derive(Deserialize)]
pub struct EditProfileUpdateRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub ib_github: String,
  pub ib_ibp: String,
  pub ib_pro: String,
  pub ib_services: String,
  pub ib_location: String,
  pub ib_website: String,
}

#[derive(Deserialize)]
pub struct SearchUsersRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub query: String,
}

#[derive(Deserialize)]
pub struct SearchPostsRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub tag: String,
}

#[derive(Deserialize)]
pub struct SearchProjectsRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub query: String,
}

#[derive(Deserialize)]
pub struct SearchSectionRequest {
  pub ib_uid: i64,
  pub ib_user: String,
}

#[derive(Deserialize)]
pub struct ProjectsRequest {
  pub ib_uid: i64,
  pub ib_user: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub project: String,
  pub description: String,
  pub languages: String,
  pub reinforcements: Option<String>,
  pub reinforcements_request: Option<String>,
}

#[derive(Deserialize)]
pub struct EditProjectRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub project_id: i64,
  pub project: String,
  pub description: String,
  pub languages: String,
  pub reinforcements: Option<String>,
  pub reinforcements_request: Option<String>,
}

#[derive(Deserialize)]
pub struct QuickResponseForceRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub project_id: i64,
  pub quick_response_force: Option<String>,
}

#[derive(Deserialize)]
pub struct WarRoomRequest {
  pub ib_uid: i64,
  pub ib_user: String,
}

#[derive(Deserialize)]
pub struct WarRoomPostsPageQuery {
  pub ib_uid: i64,
  pub ib_user: String,
  pub offset: Option<i64>,
  pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct FollowersPageQuery {
  pub ib_uid: i64,
  pub offset: Option<i64>,
  pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct InboxRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub target_user: Option<String>,
}

#[derive(Deserialize)]
pub struct InboxContactsPageQuery {
  pub ib_uid: i64,
  pub ib_user: String,
  pub offset: Option<i64>,
  pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct AdsAdminRequest {
  pub ib_uid: i64,
  pub ib_user: String,
}

#[derive(Deserialize)]
pub struct AdsCreateRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub imagepath: String,
  pub url: String,
}

#[derive(Deserialize)]
pub struct AdsUpdateRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub imageid: i64,
  pub imagepath: String,
  pub url: String,
}

#[derive(Deserialize)]
pub struct AdsDeleteRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub imageid: i64,
}

#[derive(Deserialize)]
pub struct AdsUserUpdateRequest {
  pub imageid: i64,
  pub url: String,
}

#[derive(Deserialize)]
pub struct AdsUserDeleteRequest {
  pub imageid: i64,
}

#[derive(Deserialize)]
pub struct PayPalReturnQuery {
  pub token: Option<String>,
  pub subscription_id: Option<String>,
  pub ba_token: Option<String>,
}

#[derive(Deserialize)]
pub struct DMMessageRequest {
  pub target_user: String,
  pub message: String,
}

#[derive(Deserialize)]
pub struct DMMessagesRequest {
  pub target_user: String,
  pub before_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct FollowRequest {
  pub target_user: String,
}

#[derive(Deserialize)]
pub struct AckPostRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub pid: String,
}

#[derive(Deserialize)]
pub struct PostsPageQuery {
  pub ib_uid: i64,
  pub ib_user: String,
  pub before_timestamp: Option<String>,
}

#[derive(Serialize)]
pub struct PostsPageResponse {
  pub posts_html: String,
  pub has_more: bool,
}

#[derive(Serialize)]
pub struct WarRoomPostsPageResponse {
  pub posts_html: String,
  pub has_more: bool,
  pub next_offset: usize,
}

#[derive(Serialize)]
pub struct FollowersPageResponse {
  pub followers_html: String,
  pub has_more: bool,
  pub next_offset: usize,
}

#[derive(Serialize)]
pub struct InboxContactsPageResponse {
  pub contacts_html: String,
  pub has_more: bool,
  pub next_offset: usize,
}

pub struct WarRoomPostsChunk {
  pub posts_html: String,
  pub has_more: bool,
  pub next_offset: usize,
  pub total_followers: usize,
}

pub struct FollowersChunk {
  pub followers_html: String,
  pub has_more: bool,
  pub next_offset: usize,
  pub total_followers: usize,
}

#[derive(sqlx::FromRow)]
pub struct EditProfileRow {
  pub ib_ibp: String,
  pub ib_pro: String,
  pub ib_services: String,
  pub ib_location: String,
  pub ib_website: String,
}

#[derive(sqlx::FromRow)]
pub struct EditPostRow {
  pub post: String,
}

#[derive(sqlx::FromRow)]
pub struct FollowLookupRow {
  pub username: String,
  pub followers: String,
}

#[derive(sqlx::FromRow)]
pub struct UserHoverLookupRow {
  pub ib_uid: String,
  pub username: String,
  pub followers: String,
  pub total_acknowledgments: i64,
}

#[derive(sqlx::FromRow)]
pub struct SessionUserRow {
  pub username: String,
}

#[derive(sqlx::FromRow)]
pub struct MessageUserLookupRow {
  pub ib_uid: String,
  pub username: String,
}

#[derive(sqlx::FromRow)]
pub struct DMMessageRow {
  pub id: i64,
  pub sender_uid: i64,
  pub sender_username: String,
  pub recipient_username: String,
  pub message: Vec<u8>,
  pub created_at: String,
}

#[derive(sqlx::FromRow)]
pub struct DMUnreadCountRow {
  pub unread_count: i64,
}

#[derive(sqlx::FromRow)]
pub struct ConversationUsernameRow {
  pub username: String,
}

#[derive(sqlx::FromRow)]
pub struct ProjectProfileRow {
  pub id: i64,
  pub ib_uid: i64,
  pub username: String,
  pub total_acknowledgments: i64,
  pub project: String,
  pub description: String,
  pub languages: String,
  pub updated_at: String,
  pub reinforcements: Option<String>,
  pub reinforcements_request: Option<bool>,
}

#[derive(sqlx::FromRow)]
pub struct ProjectReinforcementsRow {
  pub ib_uid: i64,
  pub reinforcements: String,
  pub reinforcements_request: bool,
}

#[derive(Serialize)]
pub struct PostResponse {
  pub success: bool,
  pub message: String,
  pub postid: Option<String>,
}

#[derive(Serialize)]
pub struct DMSendResponse {
  pub success: bool,
  pub message: String,
}

#[derive(Serialize)]
pub struct DMMessageResponseItem {
  pub id: i64,
  pub sender_user: String,
  pub recipient_user: String,
  pub message: String,
  pub timestamp: String,
  pub is_mine: bool,
}

#[derive(Serialize)]
pub struct DMMessagesResponse {
  pub success: bool,
  pub messages: Vec<DMMessageResponseItem>,
  pub has_more: bool,
}

#[derive(Serialize)]
pub struct DMUnreadCountResponse {
  pub success: bool,
  pub unread_count: i64,
}

pub static TEMPLATES: LazyLock<Tera> = LazyLock::new(|| {
    Tera::new("templates/**/*").expect("Error initializing Tera")
});
pub struct RankInfo {
  pub level: i64,
  pub name: &'static str,
  pub asset: &'static str,
  pub threshold: i64,
}

pub const RANK_TABLE: &[RankInfo] = &[
  RankInfo { level: 11, name: "General", asset: "gen.svg", threshold: 10001 },
  RankInfo { level: 10, name: "Commander", asset: "cdr.svg", threshold: 9001 },
  RankInfo { level: 9, name: "Lieutenant", asset: "lt.svg", threshold: 8001 },
  RankInfo { level: 8, name: "Sergeant Major", asset: "sgm.svg", threshold: 7001 },
  RankInfo { level: 7, name: "First Sergeant", asset: "1sg.svg", threshold: 6001 },
  RankInfo { level: 6, name: "Master Sergeant", asset: "msg.svg", threshold: 5001 },
  RankInfo { level: 5, name: "Sergeant First Class", asset: "sfc.svg", threshold: 4001 },
  RankInfo { level: 4, name: "Staff Sergeant", asset: "ssg.svg", threshold: 3001 },
  RankInfo { level: 3, name: "Sergeant", asset: "sgt.svg", threshold: 2001 },
  RankInfo { level: 2, name: "Corporal", asset: "cpl.svg", threshold: 1001 },
  RankInfo { level: 1, name: "Private", asset: "pvt.svg", threshold: 0 },
];


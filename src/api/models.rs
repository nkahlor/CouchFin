use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResponse {
    pub user: User,
    pub session_info: SessionInfo,
    pub access_token: String,
    pub server_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    pub name: Option<String>,
    pub server_id: Option<String>,
    pub server_name: Option<String>,
    pub id: String,
    pub primary_image_tag: Option<String>,
    pub has_password: bool,
    pub has_configured_password: bool,
    pub has_configured_easy_password: bool,
    pub enable_auto_login: Option<bool>,
    pub last_login_date: Option<String>,
    pub last_activity_date: Option<String>,
    pub configuration: Option<UserConfiguration>,
    pub policy: Option<UserPolicy>,
    pub primary_image_aspect_ratio: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserConfiguration {
    pub audio_language_preference: Option<String>,
    pub play_default_audio_track: bool,
    pub subtitle_language_preference: Option<String>,
    pub display_missing_episodes: bool,
    #[serde(default)]
    pub grouped_folders: Vec<String>,
    pub subtitle_mode: String,
    pub display_collections_view: bool,
    pub enable_local_password: bool,
    #[serde(default)]
    pub ordered_views: Vec<String>,
    #[serde(default)]
    pub latest_items_excludes: Vec<String>,
    #[serde(default)]
    pub my_media_excludes: Vec<String>,
    pub hide_played_in_latest: bool,
    pub remember_audio_selections: bool,
    pub remember_subtitle_selections: bool,
    pub enable_next_episode_auto_play: bool,
    pub cast_receiver_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserPolicy {
    pub is_administrator: bool,
    pub is_hidden: bool,
    pub enable_collection_management: bool,
    pub enable_subtitle_management: bool,
    pub enable_lyric_management: bool,
    pub is_disabled: bool,
    pub max_parental_rating: Option<i32>,
    pub max_parental_sub_rating: Option<i32>,
    #[serde(default)]
    pub blocked_tags: Vec<String>,
    #[serde(default)]
    pub allowed_tags: Vec<String>,
    pub enable_user_preference_access: bool,
    #[serde(default)]
    pub access_schedules: Vec<AccessSchedule>,
    #[serde(default)]
    pub block_unrated_items: Vec<String>,
    pub enable_remote_control_of_other_users: bool,
    pub enable_shared_device_control: bool,
    pub enable_remote_access: bool,
    pub enable_live_tv_management: bool,
    pub enable_live_tv_access: bool,
    pub enable_media_playback: bool,
    pub enable_audio_playback_transcoding: bool,
    pub enable_video_playback_transcoding: bool,
    pub enable_playback_remuxing: bool,
    pub force_remote_source_transcoding: bool,
    pub enable_content_deletion: bool,
    #[serde(default)]
    pub enable_content_deletion_from_folders: Vec<String>,
    pub enable_content_downloading: bool,
    pub enable_sync_transcoding: bool,
    pub enable_media_conversion: bool,
    #[serde(default)]
    pub enabled_devices: Vec<String>,
    pub enable_all_devices: bool,
    #[serde(default)]
    pub enabled_channels: Vec<String>,
    pub enable_all_channels: bool,
    #[serde(default)]
    pub enabled_folders: Vec<String>,
    pub enable_all_folders: bool,
    pub invalid_login_attempt_count: i32,
    pub login_attempts_before_lockout: i32,
    pub max_active_sessions: i32,
    pub enable_public_sharing: bool,
    #[serde(default)]
    pub blocked_media_folders: Vec<String>,
    #[serde(default)]
    pub blocked_channels: Vec<String>,
    pub remote_client_bitrate_limit: i32,
    pub authentication_provider_id: String,
    pub password_reset_provider_id: String,
    pub sync_play_access: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AccessSchedule {
    pub id: i32,
    pub user_id: String,
    pub day_of_week: String,
    pub start_hour: f64,
    pub end_hour: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionInfo {
    pub play_state: Option<PlayerState>,
    #[serde(default)]
    pub additional_users: Vec<SessionUser>,
    pub capabilities: Option<ClientCapabilities>,
    pub remote_end_point: Option<String>,
    #[serde(default)]
    pub playable_media_types: Vec<String>,
    pub id: Option<String>,
    pub user_id: String,
    pub user_name: Option<String>,
    pub client: Option<String>,
    pub last_activity_date: String,
    pub last_playback_check_in: String,
    pub last_paused_date: Option<String>,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub now_playing_item: Option<BaseItem>,
    pub now_viewing_item: Option<BaseItem>,
    pub device_id: Option<String>,
    pub application_version: Option<String>,
    pub transcoding_info: Option<TranscodingInfo>,
    pub is_active: bool,
    pub supports_media_control: bool,
    pub supports_remote_control: bool,
    #[serde(default)]
    pub now_playing_queue: Vec<QueueItem>,
    pub has_custom_device_name: bool,
    pub playlist_item_id: Option<String>,
    pub server_id: Option<String>,
    pub user_primary_image_tag: Option<String>,
    #[serde(default)]
    pub supported_commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayerState {
    pub position_ticks: Option<i64>,
    pub can_seek: bool,
    pub is_paused: bool,
    pub is_muted: bool,
    pub volume_level: Option<i32>,
    pub audio_stream_index: Option<i32>,
    pub subtitle_stream_index: Option<i32>,
    pub media_source_id: Option<String>,
    pub play_method: Option<String>,
    pub repeat_mode: String,
    pub playback_order: String,
    pub live_stream_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionUser {
    pub user_id: String,
    pub user_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClientCapabilities {
    #[serde(default)]
    pub playable_media_types: Vec<String>,
    #[serde(default)]
    pub supported_commands: Vec<String>,
    pub supports_media_control: bool,
    pub supports_persistent_identifier: bool,
    pub app_store_url: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TranscodingInfo {
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub container: Option<String>,
    pub is_video_direct: bool,
    pub is_audio_direct: bool,
    pub bitrate: Option<i32>,
    pub framerate: Option<f32>,
    pub completion_percentage: Option<f64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub audio_channels: Option<i32>,
    pub hardware_acceleration_type: Option<String>,
    #[serde(default)]
    pub transcode_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueueItem {
    pub id: String,
    pub playlist_item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItem {
    pub name: Option<String>,
    pub original_title: Option<String>,
    pub server_id: Option<String>,
    pub id: String,
    pub etag: Option<String>,
    pub date_created: Option<String>,
    pub can_delete: Option<bool>,
    pub can_download: Option<bool>,
    pub has_subtitles: Option<bool>,
    pub container: Option<String>,
    pub sort_name: Option<String>,
    pub premiere_date: Option<String>,
    #[serde(default)]
    pub external_urls: Vec<ExternalUrl>,
    #[serde(default)]
    pub media_sources: Vec<MediaSourceInfo>,
    pub critic_rating: Option<f32>,
    pub path: Option<String>,
    pub official_rating: Option<String>,
    pub overview: Option<String>,
    #[serde(default)]
    pub taglines: Vec<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    pub community_rating: Option<f32>,
    pub run_time_ticks: Option<i64>,
    pub production_year: Option<i32>,
    pub index_number: Option<i32>,
    pub index_number_end: Option<i32>,
    pub parent_index_number: Option<i32>,
    #[serde(default)]
    pub provider_ids: HashMap<String, Option<String>>,
    #[serde(rename = "IsHD")]
    pub is_hd: Option<bool>,
    pub is_folder: Option<bool>,
    pub parent_id: Option<String>,
    #[serde(rename = "Type")]
    pub item_type: String,
    #[serde(default)]
    pub people: Vec<BaseItemPerson>,
    #[serde(default)]
    pub studios: Vec<NameIdPair>,
    #[serde(default)]
    pub genre_items: Vec<NameIdPair>,
    pub parent_backdrop_item_id: Option<String>,
    #[serde(default)]
    pub parent_backdrop_image_tags: Vec<String>,
    pub user_data: Option<UserItemData>,
    pub recursive_item_count: Option<i32>,
    pub child_count: Option<i32>,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub season_id: Option<String>,
    pub season_name: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub primary_image_aspect_ratio: Option<f64>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub artist_items: Vec<NameIdPair>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub album_primary_image_tag: Option<String>,
    pub album_artist: Option<String>,
    pub collection_type: Option<String>,
    pub series_primary_image_tag: Option<String>,
    #[serde(default)]
    pub media_streams: Vec<MediaStream>,
    pub video_type: Option<String>,
    #[serde(default)]
    pub image_tags: HashMap<String, String>,
    #[serde(default)]
    pub backdrop_image_tags: Vec<String>,
    pub parent_logo_item_id: Option<String>,
    pub parent_logo_image_tag: Option<String>,
    pub parent_thumb_item_id: Option<String>,
    pub parent_thumb_image_tag: Option<String>,
    pub parent_primary_image_item_id: Option<String>,
    pub parent_primary_image_tag: Option<String>,
    /// Image type (`Primary`, `Backdrop`, ...) -> image tag -> blurhash.
    #[serde(default)]
    pub image_blur_hashes: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub chapters: Vec<ChapterInfo>,
    pub location_type: Option<String>,
    pub media_type: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NameIdPair {
    pub name: Option<String>,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExternalUrl {
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItemPerson {
    pub name: Option<String>,
    pub id: String,
    pub role: Option<String>,
    #[serde(rename = "Type")]
    pub person_type: String,
    pub primary_image_tag: Option<String>,
    #[serde(default)]
    pub image_blur_hashes: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserItemData {
    pub rating: Option<f64>,
    pub played_percentage: Option<f64>,
    pub unplayed_item_count: Option<i32>,
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub is_favorite: bool,
    pub likes: Option<bool>,
    pub last_played_date: Option<String>,
    pub played: bool,
    pub key: String,
    pub item_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChapterInfo {
    pub start_position_ticks: i64,
    pub name: Option<String>,
    pub image_path: Option<String>,
    pub image_date_modified: String,
    pub image_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSourceInfo {
    pub protocol: String,
    pub id: Option<String>,
    pub path: Option<String>,
    #[serde(rename = "Type")]
    pub source_type: String,
    pub container: Option<String>,
    pub size: Option<i64>,
    pub name: Option<String>,
    pub is_remote: bool,
    pub e_tag: Option<String>,
    pub run_time_ticks: Option<i64>,
    pub supports_transcoding: bool,
    pub supports_direct_stream: bool,
    pub supports_direct_play: bool,
    pub is_infinite_stream: bool,
    pub requires_opening: bool,
    pub open_token: Option<String>,
    pub requires_closing: bool,
    pub live_stream_id: Option<String>,
    pub video_type: Option<String>,
    #[serde(default)]
    pub media_streams: Vec<MediaStream>,
    #[serde(default)]
    pub media_attachments: Vec<MediaAttachment>,
    #[serde(default)]
    pub formats: Vec<String>,
    pub bitrate: Option<i32>,
    #[serde(default)]
    pub required_http_headers: HashMap<String, Option<String>>,
    pub transcoding_url: Option<String>,
    pub transcoding_sub_protocol: Option<String>,
    pub transcoding_container: Option<String>,
    pub default_audio_stream_index: Option<i32>,
    pub default_subtitle_stream_index: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStream {
    pub codec: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub display_title: Option<String>,
    pub video_range: Option<String>,
    pub video_range_type: Option<String>,
    pub channel_layout: Option<String>,
    pub bit_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub channels: Option<i32>,
    pub sample_rate: Option<i32>,
    pub is_default: bool,
    pub is_forced: bool,
    pub is_hearing_impaired: Option<bool>,
    pub is_interlaced: bool,
    pub height: Option<i32>,
    pub width: Option<i32>,
    pub average_frame_rate: Option<f32>,
    pub real_frame_rate: Option<f32>,
    pub profile: Option<String>,
    #[serde(rename = "Type")]
    pub stream_type: String,
    pub aspect_ratio: Option<String>,
    pub index: i32,
    pub is_external: bool,
    pub delivery_method: Option<String>,
    pub delivery_url: Option<String>,
    pub is_external_url: Option<bool>,
    pub is_text_subtitle_stream: bool,
    pub supports_external_stream: bool,
    pub path: Option<String>,
    pub pixel_format: Option<String>,
    pub level: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaAttachment {
    pub codec: Option<String>,
    pub codec_tag: Option<String>,
    pub comment: Option<String>,
    pub index: i32,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub delivery_url: Option<String>,
}

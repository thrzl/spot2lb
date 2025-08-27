use serde_derive::{Deserialize, Serialize};

// from rspotify
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListenType {
    Single,
    PlayingNow,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct Scrobble {
    pub listen_type: ListenType,
    pub payload: Vec<Payload>,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct Payload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listened_at: Option<i64>,
    pub track_metadata: TrackMetadata,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct TrackMetadata {
    pub additional_info: AdditionalInfo,
    pub artist_name: String,
    pub track_name: String,
    pub release_name: String,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct AdditionalInfo {
    pub release_mbid: Option<String>,
    pub artist_mbids: Option<Vec<String>>,
    pub recording_mbid: Option<String>,
    pub artist_names: Vec<String>,
    pub discnumber: i32,
    pub duration_ms: i64,
    pub isrc: String,
    pub music_service: String,
    pub origin_url: String,
    pub release_artist_names: Vec<String>,
    pub spotify_album_artist_ids: Vec<String>,
    pub spotify_album_id: String,
    pub spotify_artist_ids: Vec<String>,
    pub spotify_id: String,
    pub submission_client: String,
    pub tracknumber: u32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ListenBrainzMBIDLookup {
    pub artist_credit_name: String,
    pub artist_mbids: Vec<String>,
    pub recording_mbid: String,
    pub recording_name: String,
    pub release_mbid: String,
    pub release_name: String,
}

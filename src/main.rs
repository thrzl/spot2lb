mod listenbrainz_models;

use anyhow::anyhow;
use chrono::{TimeDelta, Utc};
use std::env;

use listenbrainz_models::{AdditionalInfo, Payload, Scrobble, TrackMetadata};
use reqwest::Client;
use reqwest::header::HeaderMap;
use rspotify::model::{
    AdditionalType, CurrentPlaybackContext, CurrentlyPlayingType, PlayableItem, RepeatState, Token,
};
use rspotify::prelude::OAuthClient;
use rspotify::{AuthCodeSpotify, Credentials, OAuth};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;

use log::{error, info};

use crate::listenbrainz_models::{ListenBrainzMBIDLookup, ListenType};

#[derive(Clone)]
struct TrackState {
    id: String,
    progress_ms: i64,
    scrobbled: bool,
    duration: i64,
}

impl TrackState {
    fn should_scrobble(&self, track: TrackState, is_looping: bool) -> bool {
        // should scrobble if...
        //  - new track (track id != old track id)
        //  - track progress is further than 50%
        //  - is looped
        let looped = track.progress_ms <= 5500 && is_looping;
        if track.id.to_string() == self.id {
            if (self.scrobbled && looped)
                || (!self.scrobbled && track.progress_ms >= track.duration / 2)
            {
                return true;
            }
        } else {
            return true;
        }
        return false;
    }
}

// ---------------- Main Service ----------------

struct Service {
    client: Client,
    last_track_state: Option<TrackState>,
    spotify: AuthCodeSpotify,
}

impl Service {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            if let Some(playback) = self.fetch_spotify_now_playing().await? {
                let raw_item = playback.item.clone().unwrap();
                let track = if let PlayableItem::Track(track) = raw_item {
                    track
                } else {
                    error!("skipping non-track item");
                    return Err(anyhow!("track type was episode"));
                };

                let track_id = &track.id.unwrap().to_string();
                let last_track_state = self.last_track_state.clone();
                let is_looping = playback.repeat_state != RepeatState::Off;

                let current_progress = playback
                    .progress
                    .unwrap_or(TimeDelta::zero())
                    .num_milliseconds();

                let current_track_state = TrackState {
                    id: track_id.to_owned(),
                    progress_ms: current_progress,
                    scrobbled: self
                        .last_track_state
                        .as_ref()
                        .is_some_and(|state| state.scrobbled),
                    duration: track.duration.num_milliseconds(),
                };

                if last_track_state.is_none_or(|last_track| {
                    last_track.should_scrobble(current_track_state, is_looping)
                }) {
                    let is_over_half_played =
                        current_progress > (track.duration.num_milliseconds() / 2);

                    info!(
                        "sending {event_type} event to listenbrainz",
                        event_type = if is_over_half_played {
                            "listen"
                        } else {
                            "now_playing"
                        }
                    );

                    self.submit_listenbrainz(&playback, !is_over_half_played)
                        .await?;

                    self.last_track_state = Some(TrackState {
                        id: track_id.to_owned(),
                        progress_ms: current_progress,
                        scrobbled: is_over_half_played,
                        duration: track.duration.num_milliseconds(),
                    });
                }
            }

            sleep(Duration::from_secs(5)).await;
        }
    }

    async fn fetch_spotify_now_playing(&self) -> anyhow::Result<Option<CurrentPlaybackContext>> {
        let raw_now_playing = self
            .spotify
            .current_playback(None, None::<Vec<&AdditionalType>>)
            .await
            .unwrap();
        // let item = now_playing.item.unwrap();
        if raw_now_playing.is_some() {
            let now_playing = raw_now_playing.unwrap();
            if let CurrentlyPlayingType::Track = now_playing.currently_playing_type {
                Ok(Some(now_playing))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn submit_listenbrainz(
        &self,
        playback: &CurrentPlaybackContext,
        now_playing: bool,
    ) -> anyhow::Result<()> {
        let raw_item = playback.item.clone().unwrap();
        let track = if let PlayableItem::Track(track) = raw_item {
            track
        } else {
            eprintln!("skipping non-track item");
            return Err(anyhow!("track type was episode"));
        };
        let mbid_mapping_track = track.clone();

        let mut listen = Scrobble {
            listen_type: if now_playing {
                ListenType::PlayingNow
            } else {
                ListenType::Single
            },
            payload: [Payload {
                listened_at: if !now_playing {
                    Some(chrono::Utc::now().timestamp())
                } else {
                    None
                },
                track_metadata: TrackMetadata {
                    artist_name: track
                        .artists
                        .first()
                        .map(|a| a.name.clone())
                        .unwrap_or_default(),
                    track_name: track.name.clone(),
                    release_name: track.album.name,
                    additional_info: AdditionalInfo {
                        music_service: "spotify.com".to_string(),
                        submission_client: "github.com/thrzl/spotbrainz".to_string(),
                        isrc: track.external_ids.get("isrc").unwrap().to_owned(),
                        duration_ms: track.duration.num_milliseconds(),
                        artist_names: track
                            .artists
                            .iter()
                            .map(|artist| artist.name.clone())
                            .collect(),
                        spotify_artist_ids: track
                            .artists
                            .iter()
                            .map(|artist| artist.external_urls.get("spotify").unwrap().to_owned())
                            .collect(),
                        release_artist_names: track
                            .album
                            .artists
                            .iter()
                            .map(|artist| artist.name.clone())
                            .collect(),
                        spotify_album_artist_ids: track
                            .album
                            .artists
                            .iter()
                            .map(|artist| artist.external_urls.get("spotify").unwrap().to_owned())
                            .collect(),
                        spotify_album_id: track
                            .album
                            .external_urls
                            .get("spotify")
                            .unwrap()
                            .to_owned(),
                        discnumber: track.disc_number,
                        spotify_id: track.external_urls.get("spotify").unwrap().to_owned(),
                        origin_url: track.external_urls.get("spotify").unwrap().to_owned(),
                        tracknumber: track.track_number,
                        artist_mbids: None,
                        release_mbid: None,
                        recording_mbid: None,
                    },
                },
            }]
            .to_vec(),
        };

        let mbid_mapping = self
            .resolve_mbid(
                mbid_mapping_track.name,
                mbid_mapping_track
                    .artists
                    .iter()
                    .map(|artist| artist.name.to_string())
                    .collect(),
                mbid_mapping_track.album.name,
            )
            .await;

        if let Some(mapping) = mbid_mapping {
            listen.payload[0]
                .track_metadata
                .additional_info
                .artist_mbids = Some(mapping.artist_mbids);
            listen.payload[0]
                .track_metadata
                .additional_info
                .release_mbid = Some(mapping.release_mbid);
            listen.payload[0]
                .track_metadata
                .additional_info
                .recording_mbid = Some(mapping.recording_mbid);
        }
        println!("{}", serde_json::json!(listen.clone()));

        let res = self
            .client
            .post("https://api.listenbrainz.org/1/submit-listens")
            .json(&serde_json::json!(listen))
            .send()
            .await?;

        info!(
            "attempting to send {event:?}: {title} by {artist}",
            title = track.name,
            artist = track.artists.first().unwrap().name,
            event = listen.listen_type
        );

        let status = &res.status();
        let body = res.text().await.unwrap_or("no response at all".to_string());
        if status.is_success() {
            info!("successfully sent scrobble :)")
        } else {
            return Err(anyhow!("{status} {body}"));
        }

        Ok(())
    }

    async fn resolve_mbid(
        &self,
        track_name: String,
        artist_credit_name: String,
        release_name: String,
    ) -> Option<ListenBrainzMBIDLookup> {
        let res = self.client.get(format!("https://api.listenbrainz.org/1/metadata/lookup/?artist_name={artist_credit_name}&recording_name={track_name}&release_name={release_name}")).send().await.unwrap();
        let json_result = res.json().await;
        if let Ok(data) = json_result {
            data
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{:#?} {} {}] {}",
                chrono::Local::now(),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .level_for("reqwest", log::LevelFilter::Warn)
        .level_for("rspotify_http::reqwest", log::LevelFilter::Warn)
        .chain(std::io::stdout())
        .apply()?;

    let _ = dotenvy::dotenv(); // dont even bother unwrapping this should only error if the file isnt found

    let creds = Credentials {
        id: env::var("SPOTIFY_CLIENT_ID").expect("no spotify client id!"),
        secret: Some(env::var("SPOTIFY_CLIENT_SECRET").expect("no spotify client secret!")),
    };
    let refresh_token = Token {
        access_token: "".to_string(),
        expires_at: Some(Utc::now()),
        expires_in: TimeDelta::new(0, 0).unwrap(),
        refresh_token: Some(env::var("SPOTIFY_REFRESH_TOKEN").expect("no spotify token!")),
        scopes: HashSet::new(),
    };
    let oauth = OAuth::default();
    let spot = AuthCodeSpotify::new(creds, oauth);
    {
        let mut guard = spot
            .token
            .lock()
            .await
            .expect("couldn't get a lock on the spotify token");
        *guard = Some(refresh_token);
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!(
            "Token {}",
            env::var("LISTENBRAINZ_TOKEN").expect("no listenbrainz token!")
        )
        .try_into()
        .unwrap(),
    );
    let mut service = Service {
        client: Client::builder().default_headers(headers).build().unwrap(),
        last_track_state: None,
        spotify: spot,
    };
    info!("initialized service :P");

    service.run().await
}

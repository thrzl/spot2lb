# spotify -> listenbrainz scrobbler

works in real time and tries to match tracks before scrobbling for richer now playing data from listenbrainz

built to be able to get my spotify data over websockets (the websocket connection is via listenbrainz)

very lightweight; seems to peak at ~6MB of memory usage and 5MB ingress/hour

## how to use

1. you need to set the `RSPOTIFY_REFRESH_TOKEN` (this is not a typo, this program uses the `rspotify` crate) and `LISTENBRAINZ_TOKEN` environment variables.
2. that's it it'll just work

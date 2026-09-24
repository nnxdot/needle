// Your music server as a source: Navidrome, and other servers that speak the Subsonic API
// (Airsonic-Advanced, Gonic, Ampache, LMS). Songs are listed in Needle and streamed from the
// server; plays and ratings are saved on the server too.

fn source() {
    #{
        name: "Navidrome / Subsonic",
        fields: [
            #{ id: "server", label: "Server address", placeholder: "https://music.example.com" },
            #{ id: "username", label: "User name", placeholder: "" },
            #{ id: "password", label: "Password", placeholder: "", secret: true },
        ]
    }
}

fn signed_in() {
    let server = setting("server");
    server != () && server != "" && secret("password") != ()
}

// "music.example.com/" becomes "https://music.example.com".
fn address(typed) {
    let server = typed;
    server.trim();
    while server.ends_with("/") {
        server.pop();
    }
    if server != "" && !server.starts_with("http://") && !server.starts_with("https://") {
        server = "https://" + server;
    }
    server
}

// The sign-in part of every request. The password itself is never sent: only a salted hash.
fn auth() {
    let password = secret("password");
    if password == () {
        throw "Sign in to your server first";
    }
    let salt = random_text(12);
    `u=${url_encode(setting("username"))}&t=${md5(password + salt)}&s=${salt}&v=1.16.1&c=Needle&f=json`
}

fn link(method, params, auth) {
    let url = `${setting("server")}/rest/${method}.view?${auth}`;
    for key in params.keys() {
        url += `&${key}=${url_encode(params[key].to_string())}`;
    }
    url
}

fn ask(method, params) {
    let answer = parse_json(http_get(link(method, params, auth())));
    let reply = answer["subsonic-response"];
    if reply == () {
        throw "That address did not answer like a Subsonic server";
    }
    if reply.status != "ok" {
        let message = if reply.error != () { reply.error.message } else { () };
        throw if message != () { message } else { "The server refused the request" };
    }
    reply
}

fn sign_in(fields) {
    let server = address(fields.server);
    if server == "" {
        throw "Type your server's address";
    }
    set_setting("server", server);
    set_setting("username", fields.username);
    set_secret("password", fields.password);
    try {
        ask("ping", #{});
    } catch (error) {
        sign_out();
        throw error;
    }
}

fn sign_out() {
    set_setting("server", "");
    delete_secret("password");
}

// 500 songs a page, until a page comes back empty.
fn songs(page) {
    let reply = ask("search3", #{
        query: "", songCount: 500, songOffset: page * 500, artistCount: 0, albumCount: 0
    });
    let found = if reply.searchResult3 != () { reply.searchResult3.song } else { () };
    if found == () {
        return [];
    }
    // One sign-in for all the cover links of this page, so an album's songs share one link.
    let cover_auth = auth();
    let list = [];
    for song in found {
        list.push(#{
            id: song.id,
            title: song.title,
            artist: song.artist,
            album: song.album,
            album_artist: song.displayAlbumArtist,
            genre: song.genre,
            year: song.year,
            track: song.track,
            disc: song.discNumber,
            duration: song.duration,
            format: song.suffix,
            bitrate: song.bitRate,
            sample_rate: song.samplingRate,
            bit_depth: song.bitDepth,
            channels: song.channelCount,
            size: song.size,
            musicbrainz_id: song.musicBrainzId,
            cover: if song.coverArt != () {
                link("getCoverArt", #{ id: song.coverArt, size: 600 }, cover_auth)
            } else {
                ""
            },
        });
    }
    list
}

// The original file, not a smaller copy.
fn stream(id) {
    link("stream", #{ id: id, format: "raw" }, auth())
}

fn playing(id) {
    ask("scrobble", #{ id: id, submission: false });
}

fn played(id, started_at) {
    ask("scrobble", #{ id: id, time: started_at * 1000, submission: true });
}

fn rate(id, stars) {
    ask("setRating", #{ id: id, rating: stars });
}

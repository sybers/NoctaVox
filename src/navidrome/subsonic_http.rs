//! Subsonic REST calls using HTTP GET. The `submarine` crate uses POST; some reverse proxies
//! mishandle POST query strings, which breaks token auth against Navidrome (Subsonic error 40).

use serde::Deserialize;
use submarine::{
    api::get_album_list::Order,
    auth::Auth,
    data::{AlbumWithSongsId3, Child, Info, ResponseType},
    SubsonicError,
};

#[derive(Deserialize)]
struct SubsonicEnvelope {
    #[serde(rename = "subsonic-response")]
    inner: SubsonicBody,
}

#[derive(Deserialize)]
struct SubsonicBody {
    #[serde(flatten)]
    info: Info,
    #[serde(flatten)]
    data: ResponseType,
}

fn merge_params(auth: &Auth, mut extra: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut v = Vec::with_capacity(6 + extra.len());
    v.push(("u".into(), auth.user.clone()));
    v.push(("v".into(), auth.version.clone()));
    v.push(("c".into(), auth.client_name.clone()));
    v.push(("t".into(), auth.hash.clone()));
    v.push(("s".into(), auth.salt.clone()));
    v.push(("f".into(), "json".into()));
    v.append(&mut extra);
    v
}

async fn get_json(
    http: &reqwest::Client,
    base_url: &str,
    auth: &Auth,
    path: &str,
    extra: Vec<(String, String)>,
) -> Result<SubsonicBody, SubsonicError> {
    let params = merge_params(auth, extra);
    let url = format!(
        "{}/rest/{}",
        base_url.trim_end_matches('/'),
        path.trim_matches('/')
    );
    let body = match http.get(&url).query(&params).send().await {
        Ok(resp) => match resp.status() {
            reqwest::StatusCode::OK => resp.text().await?,
            _ => return Err(SubsonicError::NoServerFound),
        },
        Err(e) => return Err(SubsonicError::Connection(e)),
    };

    let envelope: SubsonicEnvelope = serde_json::from_str(&body).map_err(SubsonicError::Conversion)?;
    match envelope.inner.data {
        ResponseType::Error { error } => Err(SubsonicError::Server(error.to_string())),
        _ => Ok(envelope.inner),
    }
}

fn create_album_list_params(
    size: Option<usize>,
    offset: Option<usize>,
    music_folder_id: Option<String>,
) -> Vec<(String, String)> {
    let mut paras = Vec::new();
    if let Some(size) = size {
        paras.push(("size".into(), size.to_string()));
    }
    if let Some(offset) = offset {
        paras.push(("offset".into(), offset.to_string()));
    }
    if let Some(folder_id) = music_folder_id {
        paras.push(("musicFolderId".into(), folder_id));
    }
    paras
}

pub(crate) async fn ping_get(
    http: &reqwest::Client,
    base_url: &str,
    auth: &Auth,
) -> Result<Info, SubsonicError> {
    let body = get_json(http, base_url, auth, "ping", vec![]).await?;
    if let ResponseType::Ping {} = body.data {
        Ok(body.info)
    } else {
        Err(SubsonicError::Submarine(String::from(
            "expected type Ping but found wrong type",
        )))
    }
}

pub(crate) async fn get_album_list2_get(
    http: &reqwest::Client,
    base_url: &str,
    auth: &Auth,
    order: Order,
    size: Option<usize>,
    offset: Option<usize>,
    music_folder_id: Option<String>,
) -> Result<Vec<Child>, SubsonicError> {
    let mut extra = create_album_list_params(size, offset, music_folder_id);
    extra.push(("type".into(), order.to_string()));
    extra.push(("openSubsonic".into(), String::from("false")));

    let body = get_json(http, base_url, auth, "getAlbumList2", extra).await?;
    if let ResponseType::AlbumList2 { album_list2 } = body.data {
        Ok(album_list2.album)
    } else {
        Err(SubsonicError::Submarine(String::from(
            "expected type AlbumList2 but found wrong type",
        )))
    }
}

pub(crate) async fn get_album_get(
    http: &reqwest::Client,
    base_url: &str,
    auth: &Auth,
    id: impl Into<String>,
) -> Result<AlbumWithSongsId3, SubsonicError> {
    let extra = vec![("id".into(), id.into())];
    let body = get_json(http, base_url, auth, "getAlbum", extra).await?;
    if let ResponseType::Album { album } = body.data {
        Ok(album)
    } else {
        Err(SubsonicError::Submarine(String::from(
            "expected type Album but found wrong type",
        )))
    }
}

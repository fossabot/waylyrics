use std::{thread, time::Duration};

use ncmapi::{
    types::{Album, Artist, Song},
    NcmApi,
};
use tokio::runtime::Handle;

use ncmapi::types::{LyricResp, SearchSongResp};

use super::Lyric;

pub struct NeteaseLyricProvider {}

pub struct NeteaseLyric {
    lyric: Option<String>,
    tlyric: Option<String>,
}

impl super::LyricProvider for NeteaseLyricProvider {
    type Id = usize;
    type LStore = NeteaseLyric;

    const NAME: &'static str = "网易云音乐";

    fn search_song(
        &self,
        handle: &Handle,
        album: &str,
        artists: &[&str],
        title: &str,
    ) -> Result<Vec<super::SongInfo<Self::Id>>, Box<dyn std::error::Error>> {
        let handle = handle.clone();
        let keyword = format!("{title} {album} {}", artists.join("/"));

        tracing::debug!("search keyword: {keyword}");

        let cookie_path = crate::CONFIG_HOME.with_borrow(|home| home.to_owned() + "ncm-cookie");
        let search_result = thread::spawn(move || {
            let api = NcmApi::new(
                false,
                Duration::from_secs(60 * 60),
                Duration::from_secs(5 * 60),
                true,
                &cookie_path,
            );
            handle.block_on(async { api.search(&keyword, None).await })
        })
        .join()
        .unwrap()?;
        let resp: SearchSongResp = search_result.deserialize()?;
        tracing::debug!("search result: {resp:?}");

        Ok(resp
            .result
            .ok_or("no search result")?
            .songs
            .iter()
            .map(
                |Song {
                     name,
                     id,
                     artists,
                     duration,
                     album: Album { name: album, .. },
                     ..
                 }| super::SongInfo {
                    id: *id as _,
                    title: name.into(),
                    album: album.clone(),
                    singer: artists
                        .iter()
                        .filter_map(|Artist { name, .. }| name.as_ref())
                        .fold(String::new(), |mut s, op| {
                            if !s.is_empty() {
                                s.push(',')
                            }
                            s += op;
                            s
                        }),
                    length: Duration::from_millis(*duration as _),
                },
            )
            .collect())
    }

    fn query_lyric(
        &self,
        handle: &Handle,
        id: Self::Id,
    ) -> Result<NeteaseLyric, Box<dyn std::error::Error>> {
        let handle = handle.clone();
        let cookie_path = crate::CONFIG_HOME.with_borrow(|home| home.to_owned() + "ncm-cookie");
        let query_result = thread::spawn(move || {
            let api = NcmApi::new(
                false,
                Duration::from_secs(60 * 60),
                Duration::from_secs(5 * 60),
                true,
                &cookie_path,
            );
            handle.block_on(async { api.lyric(id).await })
        })
        .join()
        .unwrap()?;

        let lyric_resp: LyricResp = query_result.deserialize()?;

        tracing::debug!("lyric query result: {lyric_resp:?}");

        Ok(NeteaseLyric {
            lyric: lyric_resp.lrc.map(|l| l.lyric),
            tlyric: lyric_resp.tlyric.map(|l| l.lyric),
        })
    }

    fn new() -> Result<Box<Self>, Box<dyn std::error::Error>> {
        Ok(Box::new(Self {}))
    }
}

impl super::LyricStore for NeteaseLyric {
    fn get_lyric(&self) -> Lyric<'_> {
        let lyric = self.lyric.as_deref();
        match_lyric(lyric)
    }

    fn get_translated_lyric(&self) -> Lyric<'_> {
        let lyric = self.tlyric.as_deref();
        match_lyric(lyric)
    }
}

fn match_lyric(lyric: Option<&str>) -> Lyric<'_> {
    match lyric {
        Some("") | None => super::Lyric::None,
        Some(lyric) => {
            if let Ok(parsed) = super::utils::lrc_iter(lyric, "\n") {
                Lyric::LineTimestamp(parsed)
            } else {
                Lyric::NoTimestamp
            }
        }
    }
}

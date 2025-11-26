use serde::Deserialize;
use url::Url;

use crate::{
    exports::nero::extension::extractor::Series,
    nero::extension::types::Episode,
    wasi::http::types::{Fields, Method, OutgoingRequest},
};

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

pub type SearchApiResponse = ApiResponse<Vec<AnimeData>>;
pub type AnimeApiResponse = ApiResponse<AnimeData>;
pub type EpisodesApiResponse = ApiResponse<Vec<EpisodeData>>;

#[derive(Debug, Deserialize)]
pub struct AnimeData {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub attributes: AnimeAttributes,
}

impl From<AnimeData> for Series {
    fn from(anime: AnimeData) -> Self {
        Self {
            id: anime.id,
            title: anime.attributes.canonical_title,
            poster_resource: anime
                .attributes
                .poster_image
                .and_then(|img| img.original)
                .and_then(|url| Url::parse(&url).ok())
                .map(|url| OutgoingRequest::from_url(&url, &Method::Get, Fields::new())),
            synopsis: anime.attributes.synopsis,
            type_: Some(anime.type_),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AnimeAttributes {
    #[serde(rename = "canonicalTitle")]
    pub canonical_title: String,
    pub synopsis: Option<String>,
    #[serde(rename = "posterImage")]
    pub poster_image: Option<ImageResource>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeData {
    pub id: String,
    pub attributes: EpisodeAttributes,
}

impl From<EpisodeData> for Episode {
    fn from(episode: EpisodeData) -> Self {
        Episode {
            id: episode.id,
            number: episode.attributes.number,
            title: episode.attributes.canonical_title,
            description: episode.attributes.synopsis,
            thumbnail_resource: episode
                .attributes
                .thumbnail
                .and_then(|img| img.original)
                .and_then(|url| Url::parse(&url).ok())
                .map(|url| OutgoingRequest::from_url(&url, &Method::Get, Fields::new())),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EpisodeAttributes {
    pub number: u16,
    #[serde(rename = "canonicalTitle")]
    pub canonical_title: Option<String>,
    pub synopsis: Option<String>,
    pub thumbnail: Option<ImageResource>,
}

#[derive(Debug, Deserialize)]
pub struct ImageResource {
    pub original: Option<String>,
}

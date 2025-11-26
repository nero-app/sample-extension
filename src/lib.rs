mod kitsu;
mod request;

use url::Url;

use crate::{
    exports::nero::extension::extractor::Guest,
    kitsu::{AnimeApiResponse, EpisodesApiResponse, SearchApiResponse},
    nero::extension::types::{
        Episode, EpisodesPage, FilterCategory, SearchFilter, Series, SeriesPage, Video,
    },
    request::Request,
    wasi::http::types::{ErrorCode, Method},
};

wit_bindgen::generate!({
    world: "nero:extension/extension",
    generate_all,
});

const KITSU_URL: &str = "https://kitsu.io/api/edge";
const PAGE_LIMIT: u16 = 10;

struct SampleExtension;

impl Guest for SampleExtension {
    fn filters() -> Result<Vec<FilterCategory>, ErrorCode> {
        Err(ErrorCode::InternalError(Some("Not implemented".to_owned())))
    }

    fn search(
        query: String,
        page: Option<u16>,
        filters: Vec<SearchFilter>,
    ) -> Result<SeriesPage, ErrorCode> {
        let url = format!("{KITSU_URL}/anime?filter[text]={query}");
        let request = Request::new(Method::Get, Url::parse(&url).unwrap());
        let response = request.send()?.json::<SearchApiResponse>()?;

        Ok(SeriesPage {
            series: response.data.into_iter().map(Series::from).collect(),
            has_next_page: false,
        })
    }

    fn get_series_info(series_id: String) -> Result<Series, ErrorCode> {
        let url = format!("{KITSU_URL}/anime/{series_id}");
        let request = Request::new(Method::Get, Url::parse(&url).unwrap());
        let response = request.send()?.json::<AnimeApiResponse>()?;

        Ok(response.data.into())
    }

    fn get_series_episodes(
        series_id: String,
        page: Option<u16>,
    ) -> Result<EpisodesPage, ErrorCode> {
        let page_num = page.unwrap_or(1);
        let page_index = page_num.saturating_sub(1) as u32;
        let offset = page_index * (PAGE_LIMIT as u32);

        let url = format!(
            "{KITSU_URL}/episodes?filter[mediaId]={}&page[limit]={}&page[offset]={}",
            series_id, PAGE_LIMIT, offset
        );

        let request = Request::new(Method::Get, Url::parse(&url).unwrap());
        let response = request.send()?.json::<EpisodesApiResponse>()?;

        Ok(EpisodesPage {
            episodes: response.data.into_iter().map(Episode::from).collect(),
            has_next_page: response
                .links
                .as_ref()
                .and_then(|links| links.next.as_ref())
                .is_some(),
        })
    }

    #[allow(unused_variables)]
    fn get_series_videos(series_id: String, episode_id: String) -> Result<Vec<Video>, ErrorCode> {
        Err(ErrorCode::InternalError(Some("Not implemented".to_owned())))
    }
}

export!(SampleExtension);

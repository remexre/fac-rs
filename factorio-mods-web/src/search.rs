use ::futures::Future;

/// The page number of one page of a search response.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, new, newtype_display, newtype_ref)]
pub struct PageNumber(u64);

/// The response number within a page of a search response.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, new, newtype_display, newtype_ref)]
pub struct ResponseNumber(u64);

/// A mod object returned by `API::search`.
#[derive(Clone, Debug, Deserialize, new, getters)]
pub struct SearchResponseMod {
	/// The mod ID.
	id: ::ModId,

	/// The name of the mod.
	name: ::factorio_mods_common::ModName,

	/// The authors of the mod.
	#[serde(deserialize_with = "::factorio_mods_common::deserialize_string_or_seq_string")]
	owner: Vec<::factorio_mods_common::AuthorName>,

	/// The title of the mod.
	title: ::factorio_mods_common::ModTitle,

	/// A short summary of the mod.
	summary: ::ModSummary,

	/// The URL of the GitHub repository of the mod.
	github_path: ::factorio_mods_common::Url,

	/// The URL of the homepage of the mod.
	homepage: ::factorio_mods_common::Url,

	/// The name of the mod's license.
	license_name: ::LicenseName,

	/// The URL of the mod's license.
	license_url: ::factorio_mods_common::Url,

	/// The versions of the game supported by the mod.
	game_versions: Vec<::factorio_mods_common::ModVersionReq>,

	/// The date and time at which the mod was created.
	created_at: ::DateTime,

	/// The date and time at which the mod was last updated.
	updated_at: ::DateTime,

	/// The latest release of the mod.
	latest_release: ::ModRelease,

	// current_user_rating: ???, # Unknown type

	/// The number of times the mod has been downloaded.
	downloads_count: ::DownloadCount,

	/// The tags of the mod.
	#[serde(deserialize_with = "::factorio_mods_common::deserialize_string_or_seq_string")]
	tags: Vec<::Tag>,
}

/// Constructs an iterator of search results.
pub fn search<'a>(
	client: &'a ::client::Client,
	url: ::url::Url
) -> impl ::futures::Stream<Item = ::SearchResponseMod, Error = ::Error> + 'a {
	SearchResultsStream {
		client,
		state: SearchResultsStreamState::FetchPage(url),
	}
}

/// An iterator of search results.
#[derive(Debug)]
struct SearchResultsStream<'a> {
	client: &'a ::client::Client,
	state: SearchResultsStreamState<'a>,
}

enum SearchResultsStreamState<'a> {
	// Only used for std::mem::replace()ing in state transitions
	Invalid,

	FetchPage(::url::Url),
	WaitingForPage(Box<::futures::Future<Item = SearchResponse, Error = ::Error> + 'a>),
	HavePage(SearchResponse),
	Ended,
}

impl<'a> ::std::fmt::Debug for SearchResultsStreamState<'a> {
	fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
		match *self {
			SearchResultsStreamState::Invalid =>
				fmt.debug_tuple("Invalid")
				.finish(),

			SearchResultsStreamState::FetchPage(ref url) =>
				fmt.debug_tuple("FetchPage")
				.field(url)
				.finish(),

			SearchResultsStreamState::WaitingForPage(_) =>
				fmt.debug_tuple("WaitingForPage")
				.finish(),

			SearchResultsStreamState::HavePage(ref page) =>
				fmt.debug_tuple("HavePage")
				.field(page)
				.finish(),

			SearchResultsStreamState::Ended =>
				fmt.debug_tuple("Ended")
				.finish(),
		}
	}
}

impl<'a> ::futures::Stream for SearchResultsStream<'a> {
	type Item = SearchResponseMod;
	type Error = ::Error;

	fn poll(&mut self) -> ::futures::Poll<Option<Self::Item>, Self::Error> {
		loop {
			let state = ::std::mem::replace(&mut self.state, SearchResultsStreamState::Invalid);
			match state {
				SearchResultsStreamState::Invalid =>
					unreachable!(),

				SearchResultsStreamState::FetchPage(url) => {
					let mut page_future = self.client.get_object(url);
					let result = page_future.poll();
					self.state = SearchResultsStreamState::WaitingForPage(Box::new(page_future));
					match result {
						Ok(::futures::Async::Ready(_)) =>
							(),

						Ok(::futures::Async::NotReady) =>
							return Ok(::futures::Async::NotReady),

						Err(err) => {
							self.state = SearchResultsStreamState::Ended;
							return Err(err);
						},
					}
				},

				SearchResultsStreamState::WaitingForPage(mut page_future) => {
					match page_future.poll() {
						Ok(::futures::Async::Ready(page)) =>
							self.state = SearchResultsStreamState::HavePage(page),

						Ok(::futures::Async::NotReady) => {
							self.state = SearchResultsStreamState::WaitingForPage(page_future);
							return Ok(::futures::Async::NotReady);
						},

						Err(::Error(::ErrorKind::StatusCode(_, ::hyper::StatusCode::NotFound), _)) =>
							self.state = SearchResultsStreamState::Ended,

						Err(err) => {
							self.state = SearchResultsStreamState::Ended;
							return Err(err);
						},
					}
				},

				SearchResultsStreamState::HavePage(mut page) =>
					if page.results.is_empty() {
						self.state = if let Some(next_url) = page.pagination.links.next {
							SearchResultsStreamState::FetchPage(next_url)
						}
						else {
							SearchResultsStreamState::Ended
						};
					}
					else {
						let result = page.results.remove(0);
						self.state = SearchResultsStreamState::HavePage(page);
						return Ok(::futures::Async::Ready(Some(result)));
					},

				SearchResultsStreamState::Ended => {
					self.state = SearchResultsStreamState::Ended;
					return Ok(::futures::Async::Ready(None));
				},
			}
		}
	}
}

/// A single search response.
#[derive(Debug, Deserialize)]
struct SearchResponse {
	pagination: SearchResponsePagination,
	results: Vec<SearchResponseMod>,
}

/// Pagination information in a search response.
#[derive(Debug, Deserialize)]
struct SearchResponsePagination {
	page_count: PageNumber,
	page: PageNumber,

	page_size: ResponseNumber,
	count: ResponseNumber,

	links: SearchResponsePaginationLinks,
}

/// Pagination link information in a search response.
#[derive(Debug, Deserialize)]
struct SearchResponsePaginationLinks {
	#[serde(deserialize_with = "deserialize_url")]
	prev: Option<::url::Url>,

	#[serde(deserialize_with = "deserialize_url")]
	next: Option<::url::Url>,

	#[serde(deserialize_with = "deserialize_url")]
	first: Option<::url::Url>,

	#[serde(deserialize_with = "deserialize_url")]
	last: Option<::url::Url>,
}

/// Deserializes a URL.
pub fn deserialize_url<'de, D>(deserializer: D) -> Result<Option<::url::Url>, D::Error> where D: ::serde::Deserializer<'de> {
	let url: Option<String> = ::serde::Deserialize::deserialize(deserializer)?;
	if let Some(url) = url {
		::url::Url::parse(&url).map(Some)
		.map_err(|err| ::serde::de::Error::custom(format!("invalid URL {:?}: {}", &url, ::std::error::Error::description(&err))))
	}
	else {
		Ok(None)
	}
}

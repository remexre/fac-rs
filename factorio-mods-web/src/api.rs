use ::futures::{ IntoFuture, Future };

/// Entry-point to the https://mods.factorio.com API
#[derive(Debug)]
pub struct API {
	base_url: ::url::Url,
	mods_url: ::url::Url,
	login_url: ::url::Url,
	client: ::client::Client,
}

impl API {
	/// Constructs an API object with the given parameters.
	pub fn new(
		config: Option<::hyper::client::Config<::hyper::client::UseDefaultConnector, ::hyper::Body>>,
		handle: ::tokio_core::reactor::Handle,
	) -> ::Result<API> {
		Ok(API {
			base_url: BASE_URL.clone(),
			mods_url: MODS_URL.clone(),
			login_url: LOGIN_URL.clone(),
			client: ::error::ResultExt::chain_err(::client::Client::new(config, handle), || "Could not initialize HTTP client")?,
		})
	}

	/// Searches for mods matching the given criteria.
	pub fn search<'a>(
		&'a self,
		query: &str,
		tags: &[&::TagName],
		order: Option<&SearchOrder>,
		page_size: Option<&::ResponseNumber>,
		page: Option<::PageNumber>
	) -> impl ::futures::Stream<Item = ::SearchResponseMod, Error = ::Error> + 'a {
		let tags_query = ::itertools::join(tags, ",");
		let order = order.unwrap_or(&DEFAULT_ORDER).to_query_parameter();
		let page_size = (page_size.unwrap_or(&DEFAULT_PAGE_SIZE)).to_string();
		let page = page.unwrap_or_else(|| ::PageNumber::new(1));

		let mut starting_url = self.mods_url.clone();
		starting_url.query_pairs_mut()
			.append_pair("q", query)
			.append_pair("tags", &tags_query)
			.append_pair("order", order)
			.append_pair("page_size", &page_size)
			.append_pair("page", &page.to_string());

		::search::search(&self.client, starting_url)
	}

	/// Gets information about the specified mod.
	pub fn get<'a>(&'a self, mod_name: &::factorio_mods_common::ModName) -> Box<::futures::Future<Item = ::Mod, Error = ::Error> + 'a> {
		// `Box` instead of `impl trait` because of ICE https://github.com/rust-lang/rust/issues/41297
		let mut mods_url = self.mods_url.clone();
		mods_url.path_segments_mut().unwrap().push(mod_name);
		Box::new(self.client.get_object(mods_url))
	}

	/// Logs in to the web API using the given username and password and returns a credentials object.
	pub fn login<'a>(
		&'a self,
		username: ::factorio_mods_common::ServiceUsername,
		password: &str
	) -> impl ::futures::Future<Item = ::factorio_mods_common::UserCredentials, Error = ::Error> + 'a {
		self.client.post_object(self.login_url.clone(), &[("username", &*username), ("password", password)])
		.map(|response: [::factorio_mods_common::ServiceToken; 1]| ::factorio_mods_common::UserCredentials::new(username, response[0].clone()))
	}

	/// Downloads the file for the specified mod release and returns a reader to the file contents.
	pub fn download<'a>(
		&'a self,
		release: &'a ::ModRelease,
		user_credentials: &'a ::factorio_mods_common::UserCredentials,
	) -> impl ::futures::Stream<Item = ::hyper::Chunk, Error = ::Error> + 'a {
		let release_download_url = release.download_url();

		self.base_url.join(release_download_url)
		.map_err(|err| ::ErrorKind::Parse(format!("{}/{}", self.base_url, release_download_url), err).into())
		.into_future()
		.and_then(move |mut download_url| {
			download_url.query_pairs_mut()
				.append_pair("username", user_credentials.username())
				.append_pair("token", user_credentials.token());

			self.client.get_zip(download_url.clone())
			.map(|response| (response, download_url))
		})
		.and_then(move |(response, download_url)| {
			let download_url2 = download_url.clone();
			let download_url3 = download_url.clone();

			let file_size =
				if let Some(&::hyper::header::ContentLength(file_size)) = response.headers().get() {
					file_size
				}
				else {
					future_bail!(::ErrorKind::MalformedResponse(download_url, "No Content-Length header".to_string()));
				};

			let expected_file_size = **release.file_size();
			future_ensure! {
				file_size == expected_file_size,
				::ErrorKind::MalformedResponse(download_url2, format!("Mod file has incorrect size {} bytes, expected {} bytes.", file_size, expected_file_size))
			}

			::futures::future::ok((response, download_url3))
		})
		.map(|(response, download_url)| HyperStreamWithUrlContext { stream: response.body(), url: download_url })
		.flatten_stream()
	}
}

struct HyperStreamWithUrlContext<S> {
	stream: S,
	url: ::url::Url,
}

impl<S> ::futures::Stream for HyperStreamWithUrlContext<S> where S: ::futures::Stream<Error = ::hyper::Error> {
	type Item = <S as ::futures::Stream>::Item;
	type Error = ::Error;

	fn poll(&mut self) -> ::futures::Poll<Option<Self::Item>, Self::Error> {
		self.stream.poll().map_err(|err| ::ErrorKind::HTTP(self.url.clone(), err).into())
	}
}

/// Search order
pub enum SearchOrder {
	/// A to Z
	Alphabetically,

	/// Most to least
	MostDownloaded,

	/// Newest to oldest
	RecentlyUpdated,
}

impl SearchOrder {
	/// Converts the SearchOrder to a string that can be ised in the search URL's querystring
	fn to_query_parameter(&self) -> &'static str {
		match *self {
			SearchOrder::Alphabetically => "alpha",
			SearchOrder::MostDownloaded => "top",
			SearchOrder::RecentlyUpdated => "updated",
		}
	}
}

const DEFAULT_ORDER: SearchOrder = SearchOrder::MostDownloaded;
lazy_static! {
	static ref BASE_URL: ::url::Url = ::url::Url::parse("https://mods.factorio.com/").unwrap();
	static ref MODS_URL: ::url::Url = ::url::Url::parse("https://mods.factorio.com/api/mods").unwrap();
	static ref LOGIN_URL: ::url::Url = ::url::Url::parse("https://auth.factorio.com/api-login").unwrap();
	static ref DEFAULT_PAGE_SIZE: ::ResponseNumber = ::ResponseNumber::new(25);
}

#[cfg(test)]
mod tests {
	use super::*;
	use ::futures::Stream;

	#[test]
	fn search_list_all_mods() {
		let mut core = ::tokio_core::reactor::Core::new().unwrap();
		let api = API::new(None, core.handle()).unwrap();

		let result =
			api.search("", &[], None, None, None)
			.fold(0usize, |count, _| ::futures::future::ok::<_, ::Error>(count + 1usize))
			.map(|count| {
				println!("Found {} mods", count);
				assert!(count > 1700); // 1700+ as of 2017-06-21
			});

		core.run(result).unwrap();
	}

	#[test]
	fn search_by_title() {
		let mut core = ::tokio_core::reactor::Core::new().unwrap();
		let api = API::new(None, core.handle()).unwrap();

		let result =
			api.search("bob's functions library mod", &[], None, None, None)
			.into_future()
			.map_err(|(err, _)| err)
			.map(|(mod_, _)| {
				let mod_ = mod_.unwrap();
				println!("{:?}", mod_);
				assert_eq!(&**mod_.title(), "Bob's Functions Library mod");
			});

		core.run(result).unwrap();
	}

	#[test]
	fn search_by_tag() {
		let mut core = ::tokio_core::reactor::Core::new().unwrap();
		let api = API::new(None, core.handle()).unwrap();

		let result =
			api.search("", &vec![&::TagName::new("logistics".to_string())], None, None, None)
			.into_future()
			.map_err(|(err, _)| err)
			.map(|(mod_, _)| {
				let mod_ = mod_.unwrap();
				println!("{:?}", mod_);
				let tag = mod_.tags().iter().find(|tag| &**tag.name() == "logistics").unwrap();
				println!("{:?}", tag);
			});

		core.run(result).unwrap();
	}

	#[test]
	fn search_non_existing() {
		let mut core = ::tokio_core::reactor::Core::new().unwrap();
		let api = API::new(None, core.handle()).unwrap();

		let result =
			api.search("arnavion's awesome mod", &[], None, None, None)
			.into_future()
			.map_err(|(err, _)| err)
			.map(|(mod_, _)| {
				assert!(mod_.is_none());
			});

		core.run(result).unwrap();
	}

	#[test]
	fn get() {
		let mut core = ::tokio_core::reactor::Core::new().unwrap();
		let api = API::new(None, core.handle()).unwrap();

		let mod_name = ::factorio_mods_common::ModName::new("boblibrary".to_string());

		let result =
			api.get(&mod_name)
			.map(|mod_| {
				println!("{:?}", mod_);
				assert_eq!(&**mod_.title(), "Bob's Functions Library mod");
			});

		core.run(result).unwrap();
	}
}

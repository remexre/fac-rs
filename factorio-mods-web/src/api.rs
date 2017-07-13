use ::futures::{ future, Future, Poll, Stream };

/// Entry-point to the https://mods.factorio.com API
#[derive(Debug)]
pub struct API {
	base_url: ::reqwest::Url,
	mods_url: ::reqwest::Url,
	login_url: ::reqwest::Url,
	client: ::client::Client,
}

impl API {
	/// Constructs an API object with the given parameters.
	pub fn new(
		builder: Option<::reqwest::unstable::async::ClientBuilder>,
		handle: ::tokio_core::reactor::Handle,
	) -> ::Result<API> {
		use ::error::ResultExt;

		Ok(API {
			base_url: BASE_URL.clone(),
			mods_url: MODS_URL.clone(),
			login_url: LOGIN_URL.clone(),
			client: ::client::Client::new(builder, handle).chain_err(|| "Could not initialize HTTP client")?,
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
	) -> impl Stream<Item = ::SearchResponseMod, Error = ::Error> + 'a {
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
	pub fn get<'a>(&'a self, mod_name: &::factorio_mods_common::ModName) -> Box<Future<Item = ::Mod, Error = ::Error> + 'a> {
		// TODO: `Box` instead of `impl trait` because of ICE https://github.com/rust-lang/rust/issues/41297
		let mut mod_url = self.mods_url.clone();
		mod_url.path_segments_mut().unwrap().push(mod_name);

		Box::new(
			self.client.get_object(mod_url)
			.map(|(mod_, _)| mod_))
	}

	/// Logs in to the web API using the given username and password and returns a credentials object.
	pub fn login<'a>(
		&'a self,
		username: ::factorio_mods_common::ServiceUsername,
		password: &str,
	) -> impl Future<Item = ::factorio_mods_common::UserCredentials, Error = ::Error> + 'a {
		self.client.post_object(self.login_url.clone(), &[("username", &*username), ("password", password)])
		.map(|(response, _): ([::factorio_mods_common::ServiceToken; 1], _)| ::factorio_mods_common::UserCredentials::new(username, response[0].clone()))
	}

	/// Downloads the file for the specified mod release and returns a reader to the file contents.
	pub fn download<'a>(
		&'a self,
		release: &::ModRelease,
		user_credentials: &::factorio_mods_common::UserCredentials,
	) -> Box<Stream<Item = ::reqwest::unstable::async::Chunk, Error = ::Error> + 'a> {
		// TODO: `Box` instead of `impl trait` because of ICE https://github.com/rust-lang/rust/issues/41297
		let release_download_url = release.download_url();
		let expected_file_size = *release.file_size();

		let mut download_url = match self.base_url.join(release_download_url) {
			Ok(download_url) => download_url,

			Err(err) => return
				Box::new(
					future::err(::ErrorKind::Parse(format!("{}/{}", self.base_url, release_download_url), err).into())
					.into_stream()),
		};

		download_url.query_pairs_mut()
			.append_pair("username", user_credentials.username())
			.append_pair("token", user_credentials.token());

		Box::new(
			self.client.get_zip(download_url)
			.and_then(move |(response, download_url)| {
				let file_size =
					if let Some(&::reqwest::header::ContentLength(file_size)) = response.headers().get() {
						file_size
					}
					else {
						bail!(::ErrorKind::MalformedResponse(download_url, "No Content-Length header".to_string()));
					};

				if file_size != *expected_file_size {
					bail!(
						::ErrorKind::MalformedResponse(
							download_url,
							format!("Mod file has incorrect size {} bytes, expected {} bytes.", file_size, expected_file_size)));
				}

				Ok(ResponseWithUrlContext { response, url: download_url })
			})
			.flatten_stream())
	}
}

struct ResponseWithUrlContext {
	response: ::reqwest::unstable::async::Response,
	url: ::reqwest::Url,
}

impl Stream for ResponseWithUrlContext {
	type Item = ::reqwest::unstable::async::Chunk;
	type Error = ::Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		self.response.body_mut().poll().map_err(|err| ::ErrorKind::HTTP(self.url.clone(), err).into())
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
	static ref BASE_URL: ::reqwest::Url = ::reqwest::Url::parse("https://mods.factorio.com/").unwrap();
	static ref MODS_URL: ::reqwest::Url = ::reqwest::Url::parse("https://mods.factorio.com/api/mods").unwrap();
	static ref LOGIN_URL: ::reqwest::Url = ::reqwest::Url::parse("https://auth.factorio.com/api-login").unwrap();
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
			.fold(0usize, |count, _| Ok::<_, ::Error>(count + 1usize))
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
			.then(|result| match result {
				Ok((Some(mod_), _)) => {
					println!("{:?}", mod_);
					assert_eq!(&**mod_.title(), "Bob's Functions Library mod");
					Ok(())
				},

				Ok((None, _)) =>
					unreachable!(),

				Err((err, _)) =>
					Err(err),
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
			.then(|result| match result {
				Ok((Some(mod_), _)) => {
					println!("{:?}", mod_);
					let tag = mod_.tags().iter().find(|tag| &**tag.name() == "logistics").unwrap();
					println!("{:?}", tag);
					Ok(())
				},

				Ok((None, _)) =>
					unreachable!(),

				Err((err, _)) =>
					Err(err),
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
			.then(|result| match result {
				Ok((Some(_), _)) => unreachable!(),
				Ok((None, _)) => Ok(()),
				Err((err, _)) => Err(err),
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

use ::futures::{ IntoFuture, Future, Stream };

/// Wraps a `hyper::Client` to only allow limited operations on it.
#[derive(Debug)]
pub struct Client {
	client: ::hyper::Client<::hyper_tls::HttpsConnector<::hyper::client::HttpConnector>>,
}

impl Client {
	/// Creates a new `Client` object.
	pub fn new(
		config: Option<::hyper::client::Config<::hyper::client::UseDefaultConnector, ::hyper::Body>>,
		handle: ::tokio_core::reactor::Handle,
	) -> ::Result<Client> {
		let config = config.unwrap_or_else(::hyper::Client::configure);
		let connector = ::error::ResultExt::chain_err(::hyper_tls::HttpsConnector::new(1, &handle), || "Could not create TLS connector")?;
		let client = config.connector(connector).build(&handle);

		/*
		client.redirect(::reqwest::RedirectPolicy::custom(|attempt| {
			if match attempt.url().host_str() {
				Some(host) if HOSTS_TO_ACCEPT_REDIRECTS_TO.contains(host) => true,
				_ => false,
			} {
				attempt.follow()
			}
			else {
				attempt.stop()
			}
		}));
		*/

		Ok(Client { client })
	}

	/// GETs the given URL using the given client, and deserializes the response as a JSON object.
	pub fn get_object<'a, T>(&'a self, url: ::url::Url) -> impl ::futures::Future<Item = T, Error = ::Error> + 'a
		where T: Send + 'a, for<'de> T: ::serde::Deserialize<'de> {

		let mut request = ::hyper::Request::new(::hyper::Get, url.to_string().parse().unwrap());
		request.headers_mut().set(::hyper::header::Accept::json());

		self.send(request, url.clone())
		.and_then(|response| json(response, url))
	}

	/// GETs the given URL using the given client, and returns an application/zip response.
	pub fn get_zip<'a>(&'a self, url: ::url::Url) -> impl ::futures::Future<Item = ::hyper::Response, Error = ::Error> + 'a {
		let mut request = ::hyper::Request::new(::hyper::Get, url.to_string().parse().unwrap());
		request.headers_mut().set(ACCEPT_APPLICATION_ZIP.clone());

		self.send(request, url.clone())
		.and_then(|response| expect_content_type(response, url, &APPLICATION_ZIP))
		.map(|(response, _)| response)
	}

	/// POSTs the given URL using the given client and request body, and deserializes the response as a JSON object.
	pub fn post_object<'a, B, T>(&'a self, url: ::url::Url, body: &B) -> Box<::futures::Future<Item = T, Error = ::Error> + 'a>
		where B: ::serde::Serialize, T: Send + 'a, for<'de> T: ::serde::Deserialize<'de> {

		let url2 = url.clone();
		let url3 = url.clone();

		let mut request = ::hyper::Request::new(::hyper::Post, url.to_string().parse().unwrap());

		{
			let headers = request.headers_mut();
			headers.set(::hyper::header::ContentType::form_url_encoded());
			headers.set(::hyper::header::Accept::json());
		}

		// Box because of bug in `conservative_impl_trait` that somehow requires `body` to be `'a` too
		// Repro: http://play.integer32.com/?gist=c4baba83cc00a45ddeed9b799222358f&version=nightly
		// which works when changed to not use impl trait: http://play.integer32.com/?gist=cf52c03896a6b24d48d26c365ea6a5a6&version=nightly
		Box::new(
			::serde_urlencoded::to_string(body)
			.map_err(|err| ::ErrorKind::Form(url, err).into())
			.into_future()
			.and_then(move |body| {
				request.set_body(body);
				self.send(request, url2)
				.and_then(|response| json(response, url3))
			}))
	}

	fn send<'a>(&'a self, mut request: ::hyper::Request, url: ::url::Url) -> impl ::futures::Future<Item = ::hyper::Response, Error = ::Error> + 'a {
		request.headers_mut().set(USER_AGENT.clone());

		let url2 = url.clone();

		self.client.request(request)
		.map_err(|err| ::ErrorKind::HTTP(url2, err).into())
		.and_then(|response| match response.status() {
			::hyper::StatusCode::Ok =>
				::futures::future::Either::A(::futures::future::ok(response)),

			::hyper::StatusCode::Unauthorized =>
				::futures::future::Either::B(
					json(response, url)
					.and_then(|object: LoginFailureResponse| future_bail!(::ErrorKind::LoginFailure(object.message)))),

			::hyper::StatusCode::Found =>
				::futures::future::Either::A(::futures::future::err(::ErrorKind::UnexpectedRedirect(url).into())),

			code =>
				::futures::future::Either::A(::futures::future::err(::ErrorKind::StatusCode(url, code).into())),
		})
	}
}

lazy_static! {
	static ref HOSTS_TO_ACCEPT_REDIRECTS_TO: ::std::collections::HashSet<&'static str> = vec![
		"mods.factorio.com",
		"mods-data.factorio.com",
	].into_iter().collect();
	static ref USER_AGENT: ::hyper::header::UserAgent = ::hyper::header::UserAgent::new(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")));
	static ref APPLICATION_ZIP: ::hyper::mime::Mime = "application/zip".parse().unwrap();
	static ref ACCEPT_APPLICATION_ZIP: ::hyper::header::Accept = ::hyper::header::Accept(vec![::hyper::header::qitem("application/zip".parse().unwrap())]);
}

/// A login failure response.
#[derive(Debug, Deserialize)]
struct LoginFailureResponse {
	message: String,
}

fn json<T>(response: ::hyper::Response, url: ::url::Url) -> impl ::futures::Future<Item = T, Error = ::Error>
	where T: Send, for<'de> T: ::serde::Deserialize<'de> {

	expect_content_type(response, url, &::hyper::mime::APPLICATION_JSON)
	.and_then(|(response, url)| {
		let url2 = url.clone();

		response.body()
		.concat2()
		.map(|chunk| (chunk, url))
		.map_err(|err| ::ErrorKind::HTTP(url2, err).into())
	})
	.and_then(|(chunk, url)| match ::serde_json::from_reader(&*chunk) {
		Ok(object) => ::futures::future::ok(object),
		Err(err) => ::futures::future::err(::ErrorKind::JSON(url, err).into()),
	})
}

fn expect_content_type(response: ::hyper::Response, url: ::url::Url, expected_mime: &::hyper::mime::Mime) -> impl ::futures::Future<Item = (::hyper::Response, ::url::Url), Error = ::Error> {
	#[cfg_attr(feature = "cargo-clippy", allow(never_loop))]
	loop {
		match response.headers().get() {
			Some(&::hyper::header::ContentType(ref mime)) if mime == expected_mime =>
				(),
			Some(&::hyper::header::ContentType(ref mime)) =>
				break ::futures::future::err(::ErrorKind::MalformedResponse(url, format!("Unexpected Content-Type header: {}", mime)).into()),
			None =>
				break ::futures::future::err(::ErrorKind::MalformedResponse(url, "No Content-Type header".to_string()).into()),
		};

		break ::futures::future::ok((response, url));
	}
}

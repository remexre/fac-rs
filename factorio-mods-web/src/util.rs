/// GETs the given URL using the given client, and returns the raw response.
pub fn get(client: &::hyper::Client, url: ::hyper::Url) -> ::Result<::hyper::client::Response> {
	let response = client.get(url).send()?;
	Ok(match response.status {
		::hyper::status::StatusCode::Ok =>
			response,

		::hyper::status::StatusCode::Unauthorized => {
			let object: LoginFailureResponse = ::serde_json::from_reader(response)?;
			bail!(::ErrorKind::LoginFailure(object.message))
		},

		::hyper::status::StatusCode::Found =>
			bail!(::ErrorKind::LoginFailure("Redirected to login page.".to_string())),

		code =>
			bail!(::ErrorKind::StatusCode(code)),
	})
}

/// GETs the given URL using the given client, and deserializes the response as a JSON object.
pub fn get_object<T>(client: &::hyper::Client, url: ::hyper::Url) -> ::Result<T> where T: ::serde::Deserialize {
	let response = get(client, url)?;
	let object = ::serde_json::from_reader(response)?;
	Ok(object)
}

/// POSTs the given URL using the given client and request body, and returns the raw response.
pub fn post(client: &::hyper::Client, url: ::hyper::Url, body: String) -> ::Result<::hyper::client::Response> {
	let response =
		client.post(url)
		.header(::hyper::header::ContentType::form_url_encoded())
		.body(&body)
		.send()?;

	Ok(match response.status {
		::hyper::status::StatusCode::Ok =>
			response,

		::hyper::status::StatusCode::Unauthorized => {
			let object: LoginFailureResponse = ::serde_json::from_reader(response)?;
			bail!(::ErrorKind::LoginFailure(object.message))
		},

		::hyper::status::StatusCode::Found =>
			bail!(::ErrorKind::LoginFailure("Redirected to login page.".to_string())),

		code =>
			bail!(::ErrorKind::StatusCode(code)),
	})
}

/// POSTs the given URL using the given client and request body, and deserializes the response as a JSON object.
pub fn post_object<T>(client: &::hyper::Client, url: ::hyper::Url, body: String) -> ::Result<T> where T: ::serde::Deserialize {
	let response = post(client, url, body)?;
	let object = ::serde_json::from_reader(response)?;
	Ok(object)
}

/// A login failure response.
#[derive(Debug, Deserialize)]
struct LoginFailureResponse {
	message: String,
}

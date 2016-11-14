/// Error kinds for errors returned by this crate.
#[derive(Debug, error_chain)]
pub enum ErrorKind {
	/// An error from `hyper`
	#[error_chain(foreign)]
	Hyper(::hyper::Error),

	/// Deserializing some JSON failed
	#[error_chain(foreign)]
	JSON(::serde_json::Error),

	/// Parsing a URL failed
	#[error_chain(foreign)]
	Parse(::url::ParseError),

	/// An HTTP request did not have a successful status code
	#[error_chain(custom)]
	StatusCode(::hyper::status::StatusCode),

	/// A request to the web API resulted in a login failure response
	#[error_chain(custom)]
	LoginFailure(String),

	/// Trying to download a mod from the mods portal returned a malformed response
	#[error_chain(custom)]
	MalformedModDownloadResponse(String),
}

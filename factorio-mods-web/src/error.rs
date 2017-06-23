/// Error kinds for errors returned by this crate.
#[derive(Debug, error_chain)]
pub enum ErrorKind {
	/// A generic error message
	Msg(String),

	/// Could not serialize a form body
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, _| write!(f, "Could not serialize the form body for the URL {}", url)"#)]
	#[error_chain(cause = "|_, err| err")]
	Form(::url::Url, ::serde_urlencoded::ser::Error),

	/// Could not perform HTTP request
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, _| write!(f, "Could not fetch URL {}", url)"#)]
	#[error_chain(cause = "|_, err| err")]
	HTTP(::url::Url, ::hyper::Error),

	/// Could not parse JSON
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, _| write!(f, "Could not parse response from {} as valid JSON", url)"#)]
	#[error_chain(cause = "|_, err| err")]
	JSON(::url::Url, ::serde_json::Error),

	/// Parsing a URL failed
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, _| write!(f, "Could not parse URL {}", url)"#)]
	#[error_chain(cause = "|_, err| err")]
	Parse(String, ::url::ParseError),

	/// An HTTP request did not have a successful status code
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, code| write!(f, "Request to URL {} returned {}", url, code)"#)]
	StatusCode(::url::Url, ::hyper::StatusCode),

	/// A request to the web API resulted in a login failure response
	#[error_chain(custom)]
	#[error_chain(display = r#"|message| write!(f, "Login failed: {}", message)"#)]
	LoginFailure(String),

	/// Got a malformed HTTP response
	#[error_chain(custom)]
	#[error_chain(display = r#"|url, reason| write!(f, "Request to URL {} was malformed: {}", url, reason)"#)]
	MalformedResponse(::url::Url, String),

	/// Received a redirect to a host that isn't in the allowed list
	#[error_chain(custom)]
	#[error_chain(display = r#"|url| write!(f, "Unexpected redirect to {}", url)"#)]
	UnexpectedRedirect(::url::Url),
}

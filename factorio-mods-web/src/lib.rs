//! API to interface with https://mods.factorio.com/

#![deny(missing_docs)]
#![feature(conservative_impl_trait)]

#![cfg_attr(feature = "cargo-clippy", deny(clippy, clippy_pedantic))]
#![cfg_attr(feature = "cargo-clippy", allow(
	large_enum_variant,
	missing_docs_in_private_items,
	result_unwrap_used,
	shadow_reuse,
	stutter,
	too_many_arguments,
	unseparated_literal_suffix,
	use_debug,
))]

extern crate backtrace;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate derive_struct;
extern crate factorio_mods_common;
pub extern crate futures;
pub extern crate hyper;
extern crate hyper_tls;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde_urlencoded;
pub extern crate tokio_core;
extern crate url;

macro_rules! future_bail {
	($e:expr) => {
		return ::futures::future::err($e.into());
	};
}

macro_rules! future_ensure {
	($cond:expr, $e:expr) => {
		if !($cond) {
			future_bail!($e);
		}
	}
}

mod api;
pub use api::{ API, SearchOrder };

mod client;

mod error;
pub use error::{ Error, ErrorKind, Result };

mod search;
pub use search::{ PageNumber, ResponseNumber, SearchResponseMod };

mod types;
pub use types::*;

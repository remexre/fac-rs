//! A CLI tool to manage Factorio mods.

#![feature(catch_expr, exhaustive_patterns, generators, never_type, nll, proc_macro_non_items, proc_macro_path_invoc, unrestricted_attribute_tokens)]

#![cfg_attr(feature = "cargo-clippy", deny(clippy, clippy_pedantic))]
#![cfg_attr(feature = "cargo-clippy", allow(
	const_static_lifetime,
	default_trait_access,
	indexing_slicing,
	large_enum_variant,
	similar_names,
	type_complexity,
	use_self,
))]

extern crate appdirs;
#[macro_use]
extern crate clap;
extern crate derive_error_chain;
#[macro_use]
extern crate error_chain;
extern crate factorio_mods_common;
extern crate factorio_mods_local;
extern crate factorio_mods_web;
extern crate futures_await as futures;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate multimap;
extern crate package;
extern crate petgraph;
extern crate regex;
extern crate rpassword;
extern crate rprompt;
extern crate semver;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;
extern crate term_size;
extern crate textwrap;

use factorio_mods_web::reqwest;
use futures::prelude::{ async_block, await };

mod enable_disable;
mod install;
mod list;
mod remove;
mod search;
mod show;
mod update;

mod config;
mod solve;
mod util;

#[derive(Debug, ::derive_error_chain::ErrorChain)]
pub enum ErrorKind {
	Msg(String),
}

quick_main!(|| -> Result<()> {
	std::env::set_var("RUST_BACKTRACE", "1");

	// Run everything in a separate thread because the default Windows main thread stack isn't big enough (1 MiB)
	::std::thread::spawn(|| {
		let disable_subcommand = enable_disable::DisableSubCommand;
		let enable_subcommand = enable_disable::EnableSubCommand;
		let install_subcommand = install::SubCommand;
		let list_subcommand = list::SubCommand;
		let remove_subcommand = remove::SubCommand;
		let search_subcommand = search::SubCommand;
		let show_subcommand = show::SubCommand;
		let update_subcommand = update::SubCommand;
		let mut subcommands = std::collections::HashMap::<_, &util::SubCommand>::new();
		subcommands.insert("disable", &disable_subcommand);
		subcommands.insert("enable", &enable_subcommand);
		subcommands.insert("install", &install_subcommand);
		subcommands.insert("list", &list_subcommand);
		subcommands.insert("remove", &remove_subcommand);
		subcommands.insert("search", &search_subcommand);
		subcommands.insert("show", &show_subcommand);
		subcommands.insert("update", &update_subcommand);
		let subcommands = subcommands;

		let app = clap_app!(@app (app_from_crate!())
			(@setting SubcommandRequiredElseHelp)
			(@setting VersionlessSubcommands)
			(@arg proxy: --proxy +takes_value "HTTP proxy URL")
			(@arg yes: -y --yes "Answer yes to all prompts")
			(@arg no: -n --no conflicts_with("yes") "Answer no to all prompts"));

		let app = subcommands.iter().fold(app, |app, (name, subcommand)|
			app.subcommand(subcommand.build_subcommand(clap::SubCommand::with_name(name))));

		let matches = app.get_matches();

		let client = if let Some(proxy_url) = matches.value_of("proxy") {
			let mut builder = ::reqwest::unstable::async::ClientBuilder::new();
			builder.proxy(::reqwest::Proxy::all(proxy_url).chain_err(|| "Couldn't parse proxy URL")?);
			Some(builder)
		}
		else {
			None
		};

		let prompt_override = match (matches.is_present("yes"), matches.is_present("no")) {
			(true, false) => Some(true),
			(false, true) => Some(false),
			(false, false) => None,
			(true, true) => unreachable!(),
		};

		let (subcommand_name, subcommand_matches) = matches.subcommand();
		let subcommand = subcommands[subcommand_name];

		let mut core = ::factorio_mods_web::tokio_core::reactor::Core::new().chain_err(|| "Could not create Tokio event loop")?;

		let local_api = factorio_mods_local::API::new().chain_err(|| "Could not initialize local API");
		let web_api = factorio_mods_web::API::new(client, core.handle()).chain_err(|| "Could not initialize web API");

		let result = subcommand.run(
			subcommand_matches.unwrap(),
			match local_api { Ok(ref local_api) => Ok(local_api), Err(err) => Err(err), },
			match web_api { Ok(ref web_api) => Ok(web_api), Err(err) => Err(err), },
			prompt_override);

		core.run(result)
	}).join().unwrap()
});

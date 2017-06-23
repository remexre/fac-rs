use ::futures::{ Future, Stream };

pub struct SubCommand;

impl ::util::SubCommand for SubCommand {
	fn build_subcommand<'a>(&self, subcommand: ::clap::App<'a, 'a>) -> ::clap::App<'a, 'a> {
		clap_app!(@app (subcommand)
			(about: "Search the mods database.")
			(@arg query: index(1) "search string"))
	}

	fn run<'a, 'b, 'c>(
		&'a self,
		matches: &'a ::clap::ArgMatches<'b>,
		_: ::Result<&'c ::factorio_mods_local::API>,
		web_api: ::Result<&'c ::factorio_mods_web::API>,
	) -> Box<::futures::Future<Item = (), Error = ::Error> + 'c> where 'a: 'b, 'b: 'c {
		use ::ResultExt;

		Box::new(match web_api {
			Ok(web_api) => {
				let query = matches.value_of("query").unwrap_or("");

				::futures::future::Either::A(
					web_api.search(query, &[], None, None, None)
					.for_each(|mod_| {
						println!("{}", mod_.title());
						println!("    Name: {}", mod_.name());
						println!("    Tags: {}", ::itertools::join(mod_.tags().iter().map(|t| t.name()), ", "));
						println!("");
						::util::wrapping_println(mod_.summary(), "    ");
						println!("");

						::futures::future::ok(())
					})
					.or_else(|err| Err(err).chain_err(|| "Could not retrieve mods")))
			},

			Err(err) =>
				::futures::future::Either::B(::futures::future::err(err))
		})
	}
}

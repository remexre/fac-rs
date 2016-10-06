use util;

pub struct SubCommand;

impl util::SubCommand for SubCommand {
	fn build_subcommand<'a>(&self, subcommand: ::clap::App<'a, 'a>) -> ::clap::App<'a, 'a> {
		subcommand
			.about("Search the mods database.")
			.arg(
				::clap::Arg::with_name("query")
					.help("search string")
					.index(1)
					.required(true))
	}

	fn run<'a>(&self, matches: &::clap::ArgMatches<'a>, api: ::factorio_mods_api::API) {
		let query = matches.value_of("query").unwrap();

		let max_width = ::term_size::dimensions().map(|(w, _)| w);

		let iter = api.search(query, &vec![], None, None, None).unwrap();
		for mod_ in iter {
			let mod_ = mod_.unwrap();
			println!("{}", mod_.title.0);
			println!("    Name: {}", mod_.name.0);
			println!("    Tags: {}", ::itertools::join(mod_.tags.iter().map(|t| &t.name.0), ", "));
			println!("");
			max_width.map_or_else(|| {
				println!("    {}", mod_.summary.0);
			}, |max_width| {
				util::wrapping_println(mod_.summary.0.as_str(), "    ", max_width);
			});
			println!("");
		}
	}
}

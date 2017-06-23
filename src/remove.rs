use ::futures::IntoFuture;

pub struct SubCommand;

impl ::util::SubCommand for SubCommand {
	fn build_subcommand<'a>(&self, subcommand: ::clap::App<'a, 'a>) -> ::clap::App<'a, 'a> {
		clap_app!(@app (subcommand)
			(about: "Remove mods.")
			(@arg mods: ... +required index(1) "mod names to remove"))
	}

	fn run<'a, 'b, 'c>(
		&'a self,
		matches: &'a ::clap::ArgMatches<'b>,
		local_api: ::Result<&'c ::factorio_mods_local::API>,
		web_api: ::Result<&'c ::factorio_mods_web::API>,
	) -> Box<::futures::Future<Item = (), Error = ::Error> + 'c> where 'a: 'b, 'b: 'c {
		Box::new((do catch {
			let local_api = local_api?;
			let web_api = web_api?;

			let mods = matches.values_of("mods").unwrap();

			let config = ::config::Config::load(&local_api)?;
			let mut reqs = config.mods().clone();
			for mod_ in mods {
				let name = ::factorio_mods_common::ModName::new(mod_.to_string());
				reqs.remove(&name);
			}

			if ::solve::compute_and_apply_diff(&local_api, &web_api, &reqs)? {
				let config = config.with_mods(reqs);
				config.save()?;
			}

			Ok(())
		})
		.into_future())
	}
}

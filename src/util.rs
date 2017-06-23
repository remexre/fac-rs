use ::futures::Future;

pub trait SubCommand {
	fn build_subcommand<'a>(&self, subcommand: ::clap::App<'a, 'a>) -> ::clap::App<'a, 'a>;
	fn run<'a, 'b, 'c>(
		&'a self,
		matches: &'a ::clap::ArgMatches<'b>,
		local_api: ::Result<&'c ::factorio_mods_local::API>,
		web_api: ::Result<&'c ::factorio_mods_web::API>,
	) -> Box<::futures::Future<Item = (), Error = ::Error> + 'c> where 'a: 'b, 'b: 'c;
}

pub fn wrapping_println(s: &str, indent: &str) {
	match ::term_size::dimensions() {
		Some((width, _)) => {
			let wrapper = ::textwrap::Wrapper {
				width,
				initial_indent: indent,
				subsequent_indent: indent,
				break_words: true,
				squeeze_whitespace: true,
				splitter: Box::new(::textwrap::NoHyphenation),
			};

			for line in wrapper.wrap(s) {
				println!("{}", line);
			}
		},

		None =>
			println!("{}{}", indent, s),
	}
}

pub fn ensure_user_credentials<'a>(local_api: &'a ::factorio_mods_local::API, web_api: &'a ::factorio_mods_web::API) ->
	Box<::futures::Future<Item = ::factorio_mods_common::UserCredentials, Error = ::Error> + 'a> {

	use ::ErrorExt;

	Box::new(match local_api.user_credentials() {
		Ok(user_credentials) =>
			::futures::future::Either::A(::futures::future::ok(user_credentials)),

		Err(err) => {
			let is_incomplete = if let ::factorio_mods_local::ErrorKind::IncompleteUserCredentials(_) = *err.kind() { true } else { false };
			let existing_username = if is_incomplete {
				if let ::factorio_mods_local::ErrorKind::IncompleteUserCredentials(ref existing_username) = *err.kind() {
					Ok(existing_username.clone())
				}
				else {
					unreachable!()
				}
			}
			else {
				Err(err)
			};

			match existing_username {
				Ok(existing_username) =>
					::futures::future::Either::B(
						::futures::future::loop_fn((), move |()| {
							println!("You need a Factorio account to download mods.");
							println!("Please provide your username and password to authenticate yourself.");
							match existing_username {
								Some(ref username) => print!("Username [{}]: ", username),
								None => print!("Username: "),
							}
							let mut stdout = ::std::io::stdout();
							match ::std::io::Write::flush(&mut stdout) {
								Ok(_) => {
									let mut username = String::new();
									match ::std::io::stdin().read_line(&mut username) {
										Ok(_) => {
											let username = username.trim().to_string();
											let username = match(username.is_empty(), &existing_username) {
												(false, _) => ::std::borrow::Cow::Owned(::factorio_mods_common::ServiceUsername::new(username)),
												(true, &Some(ref username)) => ::std::borrow::Cow::Borrowed(username),
												_ => return ::futures::future::Either::A(::futures::future::ok(::futures::future::Loop::Continue(()))),
											};

											match ::rpassword::prompt_password_stdout("Password (not shown): ") {
												Ok(password) =>
													::futures::future::Either::B(
														web_api.login(username.into_owned(), &password)
														.map(move |user_credentials| (user_credentials, local_api))
														.map_err(|err| err.chain_err(|| "Authentication error"))
														.and_then(|(user_credentials, local_api)| {
															println!("Logged in successfully.");
															match local_api.save_user_credentials(&user_credentials) {
																Ok(_) =>
																	::futures::future::ok(::futures::future::Loop::Break(user_credentials)),

																Err(err) =>
																	::futures::future::err(err.chain_err(|| "Could not save player-data.json")),
															}
														})),

												Err(err) =>
													::futures::future::Either::A(::futures::future::err(err.chain_err(|| "Could not read password")))
											}
										},

										Err(err) =>
											::futures::future::Either::A(::futures::future::err(err.chain_err(|| "Could not read from stdin")))
									}
								},

								Err(err) =>
									::futures::future::Either::A(::futures::future::err(err.chain_err(|| "Could not write to stdout")))
							}
						})),

				Err(err) =>
					::futures::future::Either::A(::futures::future::err(err.chain_err(|| "Could not read user credentials"))),
			}
		},
	})
}

pub fn prompt_continue() -> ::Result<bool> {
	use ::ResultExt;

	loop {
		let mut choice = String::new();

		print!("Continue? [y/n]: ");

		let mut stdout = ::std::io::stdout();
		::std::io::Write::flush(&mut stdout).chain_err(|| "Could not write to stdout")?;

		::std::io::stdin().read_line(&mut choice).chain_err(|| "Could not read from stdin")?;

		match choice.trim() {
			"y" | "Y" => return Ok(true),
			"n" | "N" => return Ok(false),
			_ => continue,
		}
	}
}

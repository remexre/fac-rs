pub trait SubCommand {
	fn build_subcommand<'a>(&self, subcommand: ::clap::App<'a, 'a>) -> ::clap::App<'a, 'a>;
	fn run<'a>(&self, matches: &::clap::ArgMatches<'a>, local_api: ::Result<::factorio_mods_local::API>, web_api: ::Result<::factorio_mods_web::API>) -> ::Result<()>;
}

pub fn wrapping_println(s: &str, indent: &str, max_width: usize) {
	let wrapper = ::textwrap::Wrapper {
		width: max_width,
		initial_indent: indent,
		subsequent_indent: indent,
		break_words: true,
		squeeze_whitespace: true,
		splitter: Box::new(::textwrap::NoHyphenation),
	};

	for line in wrapper.wrap(s) {
		println!("{}", line);
	}
}

pub fn ensure_user_credentials(local_api: &::factorio_mods_local::API, web_api: &::factorio_mods_web::API) -> ::Result<::factorio_mods_common::UserCredentials> {
	use ::ResultExt;

	match local_api.user_credentials() {
		Ok(user_credentials) => Ok(user_credentials),

		Err(err) => {
			if let ::factorio_mods_local::ErrorKind::IncompleteUserCredentials(ref existing_username) = *err.kind() {
				loop {
					println!("You need a Factorio account to download mods.");
					println!("Please provide your username and password to authenticate yourself.");
					match *existing_username {
						Some(ref username) => print!("Username [{}]: ", username),
						None => print!("Username: "),
					}
					let mut stdout = ::std::io::stdout();
					::std::io::Write::flush(&mut stdout).chain_err(|| "Could not write to stdout")?;

					let mut username = String::new();
					::std::io::stdin().read_line(&mut username).chain_err(|| "Could not read from stdin")?;
					let username = username.trim().to_string();
					let username = match(username.is_empty(), existing_username) {
						(false, _) => ::std::borrow::Cow::Owned(::factorio_mods_common::ServiceUsername::new(username)),
						(true, &Some(ref username)) => ::std::borrow::Cow::Borrowed(username),
						_ => continue,
					};
					let password = ::rpassword::prompt_password_stdout("Password (not shown): ").chain_err(|| "Could not read password")?;

					match web_api.login(username.into_owned(), &password) {
						Ok(user_credentials) => {
							println!("Logged in successfully.");
							local_api.save_user_credentials(&user_credentials).chain_err(|| "Could not save player-data.json")?;
							return Ok(user_credentials);
						},

						Err(err) => {
							match err.kind() {
								&::factorio_mods_web::ErrorKind::LoginFailure(ref message) => println!("Authentication error: {}", message),
								k => println!("Error: {}", k),
							}

							continue;
						},
					}
				}
			}

			Err(err).chain_err(|| "Could not read user credentials")
		},
	}
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

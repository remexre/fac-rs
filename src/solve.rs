use ::ResultExt;

/// Computes which old mods to uninstall and which new mods to install based on the given reqs.
/// Asks the user for confirmation, then applies the diff.
///
/// Returns true if the diff was successfully applied or empty.
pub fn compute_and_apply_diff(
	local_api: &::factorio_mods_local::API,
	web_api: &::factorio_mods_web::API,
	reqs: &::std::collections::HashMap<::factorio_mods_common::ModName, ::factorio_mods_common::ModVersionReq>,
) -> ::Result<bool> {
	let solution = solve(web_api, local_api.game_version().clone(), reqs)?.ok_or("No solution found.")?;

	let all_installed_mods: ::Result<::multimap::MultiMap<_, _>> =
		local_api.installed_mods().chain_err(|| "Could not enumerate installed mods")?
		.map(|mod_| mod_.map(|mod_| (mod_.info().name().clone(), mod_)).chain_err(|| "Could not process an installed mod"))
		.collect();
	let all_installed_mods = all_installed_mods.chain_err(|| "Could not enumerate installed mods")?;

	let mut to_keep = vec![];
	let mut to_uninstall = vec![];
	let mut to_install = vec![];
	let mut to_upgrade = vec![];

	for (name, installed_mods) in all_installed_mods.iter_all() {
		if let Some(release) = solution.get(name) {
			for installed_mod in installed_mods {
				if installed_mod.info().version() == release.version() {
					to_keep.push(installed_mod);
				}
				else {
					to_uninstall.push(installed_mod);
					to_upgrade.push((installed_mod, release));
				}
			}
		}
		else {
			to_uninstall.extend(installed_mods);
		}
	}

	for (name, release) in &solution {
		if let Some(installed_mods) = all_installed_mods.get_vec(name) {
			if !installed_mods.iter().any(|installed_mod| installed_mod.info().version() == release.version()) {
				to_install.push(release);
			}
		}
		else {
			to_install.push(release);
		}
	}

	to_uninstall.sort_by(|installed_mod1, installed_mod2|
		installed_mod1.info().name().cmp(installed_mod2.info().name())
		.then_with(|| installed_mod1.info().version().cmp(installed_mod2.info().version())));

	to_install.sort_by(|release1, release2|
		release1.info_json().name().cmp(release2.info_json().name())
		.then_with(|| release1.version().cmp(release2.version())));

	to_upgrade.sort_by(|&(installed_mod1, release1), &(installed_mod2, release2)|
		installed_mod1.info().name().cmp(installed_mod2.info().name())
		.then_with(|| installed_mod1.info().version().cmp(installed_mod2.info().version()))
		.then_with(|| release1.info_json().name().cmp(release2.info_json().name()))
		.then_with(|| release1.version().cmp(release2.version())));

	if !to_upgrade.is_empty() {
		println!();
		println!("The following mods will be upgraded:");
		for &(installed_mod, release) in &to_upgrade {
			println!("{} {} -> {}", installed_mod.info().name(), installed_mod.info().version(), release.version());
		}
	}

	if !to_uninstall.is_empty() {
		println!();
		println!("The following mods will be removed:");
		for installed_mod in &to_uninstall {
			println!("{} {}", installed_mod.info().name(), installed_mod.info().version());
		}
	}

	if !to_install.is_empty() {
		println!();
		println!("The following new mods will be installed:");
		for release in &to_install {
			println!("{} {}", release.info_json().name(), release.version());
		}
	}

	println!();

	if to_uninstall.is_empty() && to_install.is_empty() {
		println!("Nothing to do.");
		return Ok(true);
	}
	else {
		let mut choice = String::new();

		loop {
			print!("Continue? [y/n]: ");

			let mut stdout = ::std::io::stdout();
			::std::io::Write::flush(&mut stdout).chain_err(|| "Could not write to stdout")?;

			::std::io::stdin().read_line(&mut choice).chain_err(|| "Could not read from stdin")?;

			match choice.trim() {
				"y" | "Y" => break,
				"n" | "N" => return Ok(false),
				_ => continue,
			}
		}
	}

	let user_credentials = ::util::ensure_user_credentials(local_api, web_api)?;

	for installed_mod in to_uninstall {
		match *installed_mod.mod_type() {
			::factorio_mods_local::InstalledModType::Zipped => {
				let path = installed_mod.path();
				println!("Removing file {}", path.display());
				::std::fs::remove_file(path).chain_err(|| format!("Could not remove file {}", path.display()))?;
			},

			::factorio_mods_local::InstalledModType::Unpacked => {
				let path = installed_mod.path();
				println!("Removing directory {}", path.display());
				::std::fs::remove_dir_all(path).chain_err(|| format!("Could not remove directory {}", path.display()))?;
			},
		}
	}

	let mods_directory = local_api.mods_directory();

	for release in to_install {
		let filename = mods_directory.join(release.filename());
		let displayable_filename = filename.display().to_string();

		let mut download_filename: ::std::ffi::OsString = filename.file_name().ok_or_else(|| format!("Could not parse filename {}", displayable_filename))?.into();
		download_filename.push(".new");
		let download_filename = filename.with_file_name(download_filename);
		let download_displayable_filename = download_filename.display().to_string();

		println!("Downloading to {}", download_displayable_filename);

		{
			let parent = download_filename.parent().ok_or_else(|| format!("Filename {} is malformed", download_displayable_filename))?;
			let parent_canonicalized = parent.canonicalize().chain_err(|| format!("Filename {} is malformed", download_displayable_filename))?;
			if parent_canonicalized != mods_directory.canonicalize().chain_err(|| format!("Could not canonicalize {}", mods_directory.display()))? {
				bail!("Filename {} is malformed.", download_displayable_filename);
			}
		}

		{
			let read = web_api.download(release, &user_credentials).chain_err(|| format!("Could not download release {} {}", release.info_json().name(), release.version()))?;
			let mut reader = ::std::io::BufReader::new(read);

			let mut file = ::std::fs::OpenOptions::new();
			let mut file = file.create(true).truncate(true);
			let file = file.write(true).open(&download_filename).chain_err(|| format!("Could not open {} for writing", download_displayable_filename))?;

			let mut writer = ::std::io::BufWriter::new(file);
			::std::io::copy(&mut reader, &mut writer).chain_err(|| format!("Could not write to file {}", download_displayable_filename))?;
		}

		println!("Renaming {} to {}", download_displayable_filename, displayable_filename);
		::std::fs::rename(download_filename, filename).chain_err(|| format!("Could not rename {} to {}", download_displayable_filename, displayable_filename))?;
	}

	Ok(true)
}

fn solve(
	api: &::factorio_mods_web::API,
	game_version: ::factorio_mods_common::ReleaseVersion,
	reqs: &::std::collections::HashMap<::factorio_mods_common::ModName, ::factorio_mods_common::ModVersionReq>,
) -> ::Result<Option<::std::collections::HashMap<::factorio_mods_common::ModName, ::factorio_mods_web::ModRelease>>> {
	let mut graph = Default::default();

	{
		let mut name_to_node_indices = Default::default();

		add_installable(&mut graph, &mut name_to_node_indices, Installable::Base(::factorio_mods_common::ModName::new("base".to_string()), game_version.clone()));

		for name in reqs.keys() {
			add_mod(api, &game_version, &mut graph, &mut name_to_node_indices, name)?;
		}

		let mut edges_to_add = vec![];

		for (_, node_indices) in name_to_node_indices.iter_all() {
			for &node_index in node_indices {
				let installable = graph.node_weight(node_index).unwrap();
				for dep in installable.dependencies() {
					let dep_node_indices = name_to_node_indices.get_vec(dep.name()).unwrap();
					for &dep_node_index in dep_node_indices {
						let dep_installable = graph.node_weight(dep_node_index).unwrap();
						if dep.version().matches(dep_installable.version()) {
							edges_to_add.push((node_index, dep_node_index, *dep.required()));
						}
					}
				}
			}
		}

		for edge_to_add in edges_to_add {
			graph.update_edge(edge_to_add.0, edge_to_add.1, edge_to_add.2);
		}
	}

	loop {
		let mut node_indices_to_remove = ::std::collections::HashSet::<_>::new();

		{
			let name_to_node_indices: ::multimap::MultiMap<_, _> = graph.node_indices().map(|node_index| {
				let installable = graph.node_weight(node_index).unwrap();
				(installable.name(), node_index)
			}).collect();

			for name in reqs.keys() {
				match name_to_node_indices.get_vec(name) {
					Some(node_indices) if !node_indices.is_empty() => { },
					_ => bail!("No valid installable releases found for {}", name),
				}
			}

			node_indices_to_remove.extend(graph.node_indices().filter(|&node_index| {
				let installable = graph.node_weight(node_index).unwrap();

				for dep in installable.dependencies() {
					if *dep.required() {
						let dep_node_indices = name_to_node_indices.get_vec(dep.name()).unwrap();
						if !dep_node_indices.into_iter().any(|&dep_node_index| {
							let dep_installable = graph.node_weight(dep_node_index).unwrap();
							dep.version().matches(dep_installable.version())
						}) {
							return true
						}
					}
				}

				if let Some(req) = reqs.get(installable.name()) {
					if !req.matches(installable.version()) {
						return true
					}
				}
				else {
					if !graph.edges_directed(node_index, ::petgraph::Direction::Incoming).any(|edge| *edge.weight()) {
						return true
					}
				}

				false
			}));

			node_indices_to_remove.extend(graph.externals(::petgraph::Direction::Incoming).filter(|&node_index| {
				let installable = graph.node_weight(node_index).unwrap();
				!reqs.contains_key(installable.name())
			}));

			for (_, node_indices) in name_to_node_indices.iter_all() {
				for &node_index1 in node_indices {
					let installable1 = graph.node_weight(node_index1).unwrap();
					let neighbors1: ::std::collections::HashSet<_> = graph.neighbors(node_index1).collect();

					for &node_index2 in node_indices {
						if node_index2 > node_index1 {
							let neighbors2: ::std::collections::HashSet<_> = graph.neighbors(node_index2).collect();
							if neighbors1 == neighbors2 {
								let installable2 = graph.node_weight(node_index2).unwrap();
								if installable1.version() < installable2.version() {
									node_indices_to_remove.insert(node_index1);
								}
								else {
									node_indices_to_remove.insert(node_index2);
								}
							}
						}
					}
				}
			}
		}

		let node_indices_to_remove = ::itertools::Itertools::sorted_by(node_indices_to_remove.drain(), |i1, i2| i1.cmp(i2).reverse());
		if !node_indices_to_remove.is_empty() {
			for node_index in node_indices_to_remove {
				graph.remove_node(node_index);
			}
		}
		else {
			break
		}
	}

	let possibilities: Vec<_> = {
		let name_to_installables: ::multimap::MultiMap<_, _> =
			graph.into_nodes_edges().0.into_iter().map(|node| {
				let installable = node.weight;
				(installable.name().clone(), Some(installable))
			}).collect();

		name_to_installables.into_iter().map(|(name, mut installables)| {
			if &*name != "base" && !reqs.contains_key(&name) {
				installables.insert(0, None);
			}

			installables
		}).collect()
	};

	let possibilities: Vec<Vec<_>> = possibilities.iter().map(|p| p.iter().map(Option::as_ref).collect()).collect();
	let possibilities: Vec<_> = possibilities.iter().map(AsRef::as_ref).collect();
	let mut permutater = Permutater::new(&possibilities[..]);

	let mut values = vec![None; possibilities.len()];

	let mut best_solution = None;

	while permutater.next(&mut values) {
		let solution = values.iter().filter_map(|installable| installable.map(|installable| (installable.name(), installable))).collect();

		if is_valid(&solution) {
			best_solution =
				Some(if let Some(best_solution) = best_solution {
					let ordering = compare(&best_solution, &solution);
					match ordering {
						::std::cmp::Ordering::Less => solution,
						::std::cmp::Ordering::Equal | ::std::cmp::Ordering::Greater => best_solution,
					}
				}
				else {
					solution
				})
		}
	}

	Ok(best_solution.map(|best_solution| best_solution.into_iter().filter_map(|(name, installable)| {
		if let Installable::Mod(ref release) = *installable {
			Some((name.clone(), release.clone()))
		}
		else {
			None
		}
	}).collect()))
}

#[derive(Debug)]
enum Installable {
	Base(::factorio_mods_common::ModName, ::factorio_mods_common::ReleaseVersion),
	Mod(::factorio_mods_web::ModRelease),
}

impl Installable {
	fn name(&self) -> &::factorio_mods_common::ModName {
		match *self {
			Installable::Base(ref name, _) => name,
			Installable::Mod(ref release) => release.info_json().name(),
		}
	}

	fn version(&self) -> &::factorio_mods_common::ReleaseVersion {
		match *self {
			Installable::Base(_, ref version) => version,
			Installable::Mod(ref release) => release.version(),
		}
	}

	fn dependencies(&self) -> &[::factorio_mods_common::Dependency] {
		match *self {
			Installable::Base(..) => &[],
			Installable::Mod(ref release) => release.info_json().dependencies(),
		}
	}
}

fn add_mod(
	api: &::factorio_mods_web::API,
	game_version: &::factorio_mods_common::ReleaseVersion,
	graph: &mut ::petgraph::Graph<Installable, bool>,
	name_to_node_indices: &mut ::multimap::MultiMap<::factorio_mods_common::ModName, ::petgraph::graph::NodeIndex>,
	name: &::factorio_mods_common::ModName,
) -> ::Result<()> {
	if name_to_node_indices.contains_key(name) {
		return Ok(());
	}

	{
		let entry = name_to_node_indices.entry(name.clone());
		entry.or_insert_vec(vec![]);
	}

	match api.get(name) {
		Ok(mod_) => {
			for release in mod_.releases() {
				if release.factorio_version().matches(game_version) {
					add_installable(graph, name_to_node_indices, Installable::Mod(release.clone()));

					for dep in release.info_json().dependencies() {
						add_mod(api, game_version, graph, name_to_node_indices, dep.name())?;
					}
				}
			}

			Ok(())
		},

		Err(err) => match *err.kind() {
			::factorio_mods_web::ErrorKind::StatusCode(_, ::factorio_mods_web::reqwest::StatusCode::NotFound) => Ok(()),

			_ => Err(err).chain_err(|| format!("Could not get mod info for {}", name)),
		},
	}
}

fn add_installable(
	graph: &mut ::petgraph::Graph<Installable, bool>,
	name_to_node_indices: &mut ::multimap::MultiMap<::factorio_mods_common::ModName, ::petgraph::graph::NodeIndex>,
	installable: Installable,
) {
	name_to_node_indices.insert(installable.name().clone(), graph.add_node(installable));
}

fn is_valid(solution: &::std::collections::HashMap<&::factorio_mods_common::ModName, &Installable>) -> bool {
	for installable in solution.values() {
		for dep in installable.dependencies() {
			if let Some(installable) = solution.get(dep.name()) {
				if !dep.version().matches(installable.version()) {
					return false
				}
			}
			else if *dep.required() {
				return false
			}
		}
	}

	true
}

fn compare<'a>(
	s1: &::std::collections::HashMap<&'a ::factorio_mods_common::ModName, &'a Installable>,
	s2: &::std::collections::HashMap<&'a ::factorio_mods_common::ModName, &'a Installable>
) -> ::std::cmp::Ordering {
	for (n1, i1) in s1 {
		if let Some(i2) = s2.get(n1) {
			if i1.version() < i2.version() {
				return ::std::cmp::Ordering::Less
			}

			if i1.version() > i2.version() {
				return ::std::cmp::Ordering::Greater
			}
		}
	}

	s1.len().cmp(&s2.len()).reverse()
}

struct Permutater<'a, T> where T: 'a {
	state: Vec<usize>,
	possibilities: &'a [&'a [Option<&'a T>]],
	run_once: bool,
}

impl<'a, T> Permutater<'a, T> {
	fn new(possibilities: &'a [&'a [Option<&'a T>]]) -> Permutater<'a, T> {
		Permutater {
			state: vec![0; possibilities.len()],
			possibilities: possibilities,
			run_once: false,
		}
	}

	fn next(&mut self, values: &mut [Option<&'a T>]) -> bool {
		if self.advance(values, 0) {
			for (value_index, element_index) in self.state.iter().enumerate() {
				values[value_index] = self.possibilities[value_index][*element_index];
			}

			true
		}
		else {
			false
		}
	}

	fn advance(&mut self, values: &mut [Option<&'a T>], index: usize) -> bool {
		if index >= values.len() {
			return false
		}

		if self.run_once {
			if self.state[index] < self.possibilities[index].len() - 1 {
				self.state[index] += 1;
				true
			}
			else {
				self.state[index] = 0;
				self.advance(values, index + 1)
			}
		}
		else {
			self.run_once = true;
			true
		}
	}
}

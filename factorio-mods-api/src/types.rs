extern crate serde;
extern crate serde_json;

make_deserializable!(pub struct DateTime(pub String));

make_deserializable!(pub struct DownloadCount(pub u64));

make_deserializable!(pub struct GameVersion(pub String));

make_deserializable!(pub struct Url(pub String));

make_deserializable!(pub struct Mod {
	pub id: ModId,

	pub name: ModName,
	pub owner: AuthorNames,
	pub title: ModTitle,
	pub summary: ModDescription,

	pub github_path: Url,
	pub homepage: Url,

	pub game_versions: Vec<GameVersion>,

	pub created_at: DateTime,
	pub latest_release: ModRelease,

	pub current_user_rating: Option<serde_json::Value>,
	pub downloads_count: DownloadCount,
	pub tags: Vec<Tag>,
});

make_deserializable!(pub struct ModId(pub u64));

make_deserializable!(pub struct ModName(pub String));

make_deserializable!(pub struct AuthorNames(pub Vec<String>));

make_deserializable!(pub struct ModTitle(pub String));

make_deserializable!(pub struct ModDescription(pub String));

make_deserializable!(pub struct ModRelease {
	pub id: ReleaseId,
	pub version: ReleaseVersion,
	pub factorio_version: GameVersion,
	pub game_version: GameVersion,

	pub download_url: Url,
	pub file_name: Filename,
	pub file_size: FileSize,
	pub released_at: DateTime,

	pub downloads_count: DownloadCount,

	pub info_json: ReleaseInfo,
});

make_deserializable!(pub struct ReleaseId(pub u64));

make_deserializable!(pub struct ReleaseVersion(pub String));

make_deserializable!(pub struct Filename(pub String));

make_deserializable!(pub struct FileSize(pub u64));

make_deserializable!(pub struct ReleaseInfo {
	pub author: AuthorNames,
	/* pub description: ModDescription, # Can't represent since `description` isn't present in every ReleaseInfo */
	pub factorio_version: GameVersion,
	/* pub homepage: Url, # Can't represent since `homepage` isn't present in every ReleaseInfo */
	pub name: ModName,
	pub title: ModTitle,
	pub version: ReleaseVersion,
});

make_deserializable!(pub struct Tag {
	pub id: TagId,
	pub name: TagName,
	pub title: TagTitle,
	pub description: TagDescription,
	/* pub type: TagType, # Can't represent since `type` is a keyword */
});

make_deserializable!(pub struct TagId(pub u64));

make_deserializable!(pub struct TagName(pub String));

make_deserializable!(pub struct TagTitle(pub String));

make_deserializable!(pub struct TagDescription(pub String));

make_deserializable!(pub struct TagType(pub String));

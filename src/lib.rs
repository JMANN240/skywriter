#[macro_use] extern crate rocket;
use ring::digest::{Context, Digest, SHA256};
use serde::{Serialize, Deserialize};
use toml::value::Table;
use std::fs;
use std::io::{self, Read};
use toml::Value;
use rocket::request::{self, Outcome, Request, FromRequest};
use rocket::fs::{NamedFile, TempFile};
use rocket::http::Status;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::io::BufReader;
use data_encoding::HEXUPPER;

#[cfg(test)]
mod tests {
	use std::io::Read;
	use std::path::{Path, PathBuf};
	use std::fs::{File, read};
	use std::vec::Vec;

	const TEST_DIR_PATH_STR: &str = "test_dir";
	const TEST_FILE_PATH_STR: &str = "test_dir/test.txt";
	const INNER_DIR_PATH_STR: &str = "test_dir/inner_dir";
	const INNER_FILE_PATH_STR: &str = "test_dir/inner_dir/inner.txt";

	#[test]
	fn read_test_file_at_path() {
		let test_file_path = Path::new(TEST_FILE_PATH_STR);
		let test_file_bytes = read(test_file_path).expect(format!("Error reading test file at '{}'", TEST_FILE_PATH_STR).as_str());
		assert_eq!(test_file_bytes, vec![116, 101, 115, 116], "Test file did not contain 'test'");
	}

	#[test]
	fn read_inner_file_at_path() {
		let inner_file_path = Path::new(INNER_FILE_PATH_STR);
		let inner_file_bytes = read(inner_file_path).expect(format!("Error reading test file at '{}'", INNER_FILE_PATH_STR).as_str());
		assert_eq!(inner_file_bytes, vec![105, 110, 110, 101, 114], "Inner file did not contain 'inner'");
	}
}

#[derive(Deserialize)]
pub struct Config {
	server: ServerConfig,
	client: ClientConfig
}

impl Config {
	pub fn get_server_config(&self) -> &ServerConfig {
		&self.server
	}
	
	pub fn get_client_config(&self) -> &ClientConfig {
		&self.client
	}
}

#[derive(Deserialize)]
pub struct ServerConfig {
	file_root: String
}

impl ServerConfig {
	pub fn get_file_root(&self) -> &str {
		self.file_root.as_str()
	}
}

#[derive(Deserialize)]
pub struct ClientConfig {
	server_url: String,
	sync_seconds: u32,
	mappings: Mappings
}

impl ClientConfig {
	pub fn get_mappings(&self) -> &Mappings {
		&self.mappings
	}
	
	pub fn get_server_url(&self) -> &str {
		&self.server_url
	}
}

#[derive(Deserialize)]
pub struct Mappings {
	files: Value,
	directories: Value
}

impl Mappings {
	pub fn get_file_mappings(&self) -> &Table {
		match self.files.as_table() {
			Some(table) => {
				table
			},
			None => {
				panic!("File mappings are not a table");
			}
		}
	}
	
	pub fn get_directory_mappings(&self) -> &Table {
		match self.directories.as_table() {
			Some(table) => {
				table
			},
			None => {
				panic!("Directory mappings are not a table");
			}
		}
	}
}

pub fn parse_config_file() -> Config {
	let config_string = fs::read_to_string("Config.toml").unwrap();
	toml::from_str(&config_string).unwrap()
}

#[derive(FromForm)]
pub struct FileUpload<'r> {
	file: TempFile<'r>
}

impl<'r> FileUpload<'r> {
	pub fn take_file(self) -> TempFile<'r> {
		self.file
	}
}

#[derive(Debug)]
pub enum ExistingFileError {
	FileNotFound,
	InvalidConfig,
	InvalidPath,
	PathIsDirectory,
	CannotOpenFile
}

pub struct ExistingFile {
	file: NamedFile
}

impl ExistingFile {
	pub fn take_file(self) -> NamedFile {
		self.file
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ExistingFile {
	type Error = ExistingFileError;

	async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		match req.segments::<PathBuf>(1..) {
			Ok(virtual_path) => {
				match req.rocket().state::<Config>() {
					Some(config) => {
						let root_path = Path::new(config.get_server_config().get_file_root());
						let full_path = root_path.join(virtual_path);
						if full_path.is_file() {
							match NamedFile::open(full_path).await {
								Ok(file) => {
									return Outcome::Success(ExistingFile { file });
								},
								Err(_) => {
									return Outcome::Failure((Status::Forbidden, ExistingFileError::CannotOpenFile));
								}
							}
						} else {
							if full_path.exists() {
								return Outcome::Failure((Status::BadRequest, ExistingFileError::PathIsDirectory));
							} else {
								return Outcome::Failure((Status::NotFound, ExistingFileError::FileNotFound));
							}
						}
					},
					None => {
						return Outcome::Failure((Status::InternalServerError, ExistingFileError::InvalidConfig));
					}
				}
			},
			Err(_) => {
				return Outcome::Failure((Status::BadRequest, ExistingFileError::InvalidPath));
			}
		}
	}
}

pub fn walk_dir(root_path: &Path) -> Vec<PathBuf> {
	let mut paths = Vec::new();
	if let Ok(iter) = fs::read_dir(root_path) {
		for entry in iter {
			if let Ok(entry) = entry {
				let path = entry.path();
				if path.is_dir() {
					let mut subpaths = walk_dir(path.as_path());
					paths.append(&mut subpaths);
				} else {
					paths.push(path)
				}
			}
		}
	}
	paths
}

pub fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest, io::Error> {
	let mut context = Context::new(&SHA256);
	let mut buffer = [0; 1024];

	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		context.update(&buffer[..count]);
	}

	Ok(context.finish())
}

pub fn sha256_digest_path(path: &Path) -> String {
	let mut digest_string = "".to_string();
	if let Ok(input_file) = fs::File::open(path) {
		let file_reader = BufReader::new(input_file);
		if let Ok(digest) = sha256_digest(file_reader) {
			digest_string = HEXUPPER.encode(digest.as_ref()).to_string();
		}
	}
	digest_string
}

pub fn modified_seconds_path(path: &Path) -> u64 {
	let mut modified_seconds = 0;
	if let Ok(metadata) = fs::metadata(path) {
		if let Ok(modified_time) = metadata.modified() {
			if let Ok(duration) = modified_time.duration_since(SystemTime::UNIX_EPOCH) {
				modified_seconds = duration.as_secs();
			}
		}
	}
	modified_seconds
}

#[derive(Serialize)]
pub struct FileHeaders {
	path: String,
	seconds: u64,
	digest: String
}

impl FileHeaders {
	pub fn from_path_buf(path: PathBuf) -> Self {
		Self {
			path: path.to_string_lossy().into_owned(),
			seconds: modified_seconds_path(&path),
			digest: sha256_digest_path(&path)
		}
	}
}
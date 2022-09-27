use ring::digest::{Context, Digest, SHA256};
use serde::{Serialize, Deserialize};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf, StripPrefixError};
use std::time::SystemTime;
use std::io::BufReader;
use data_encoding::HEXUPPER;
use toml::{Value, value::Table};

#[cfg(test)]
mod tests {
	use super::{FileInfo, modified_seconds_path, sha256_digest_path};
	use std::path::Path;

	const TEST_DIR_PATH_STR: &str = "test_dir";
	const TEST_FILE_PATH_STR: &str = "test_dir/test.txt";
	const INNER_DIR_PATH_STR: &str = "test_dir/inner_dir";
	const INNER_FILE_PATH_STR: &str = "test_dir/inner_dir/inner.txt";

	#[test]
	fn get_test_file_info() {
		let test_file_path = Path::new(TEST_FILE_PATH_STR).to_path_buf();
		let test_file_info = FileInfo::from_file_path(test_file_path).expect(format!("Error creating test file info from path at '{}'", TEST_FILE_PATH_STR).as_str());
		let assert_path = Path::new(TEST_FILE_PATH_STR);
		assert_eq!(test_file_info.get_path(), assert_path);
		assert_eq!(test_file_info.get_seconds(), modified_seconds_path(assert_path));
		assert_eq!(test_file_info.get_digest(), sha256_digest_path(assert_path));
	}

	#[test]
	fn get_inner_file_info() {
		let inner_file_path = Path::new(INNER_FILE_PATH_STR).to_path_buf();
		let inner_file_info = FileInfo::from_file_path(inner_file_path).expect(format!("Error creating inner file info from path at '{}'", INNER_FILE_PATH_STR).as_str());
		let assert_path = Path::new(INNER_FILE_PATH_STR);
		assert_eq!(inner_file_info.get_path(), assert_path);
		assert_eq!(inner_file_info.get_seconds(), modified_seconds_path(assert_path));
		assert_eq!(inner_file_info.get_digest(), sha256_digest_path(assert_path));
	}

	#[test]
	fn get_test_dir_info() {
		let test_file_path = Path::new(TEST_FILE_PATH_STR).to_path_buf();
		let test_file_info = FileInfo::from_file_path(test_file_path).expect(format!("Error creating test file info from path at '{}'", TEST_FILE_PATH_STR).as_str());

		let inner_file_path = Path::new(INNER_FILE_PATH_STR).to_path_buf();
		let inner_file_info = FileInfo::from_file_path(inner_file_path).expect(format!("Error creating inner file info from path at '{}'", INNER_FILE_PATH_STR).as_str());

		let test_dir_path = Path::new(TEST_DIR_PATH_STR);
		let test_dir_infos = FileInfo::from_dir_path(test_dir_path).expect(format!("Error creating test dir infos from path at '{}'", TEST_DIR_PATH_STR).as_str());
		
		assert_eq!(test_dir_infos.len(), 2);
		assert_eq!(test_dir_infos[0], inner_file_info);
		assert_eq!(test_dir_infos[1], test_file_info);
	}

	#[test]
	fn get_inner_dir_info() {
		let inner_file_path = Path::new(INNER_FILE_PATH_STR).to_path_buf();
		let inner_file_info = FileInfo::from_file_path(inner_file_path).expect(format!("Error creating inner file info from path at '{}'", INNER_FILE_PATH_STR).as_str());

		let test_dir_path = Path::new(INNER_DIR_PATH_STR);
		let test_dir_infos = FileInfo::from_dir_path(test_dir_path).expect(format!("Error creating inner dir infos from path at '{}'", INNER_DIR_PATH_STR).as_str());
		
		assert_eq!(test_dir_infos.len(), 1);
		assert_eq!(test_dir_infos[0], inner_file_info);
	}
}

#[derive(Deserialize)]
pub struct Config {
	server: ServerConfig,
	client: ClientConfig
}

impl Config {
	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		let config_string = fs::read_to_string(path).unwrap();
		toml::from_str(&config_string).unwrap()
	}

	pub fn get_server_config(&self) -> &ServerConfig {
		&self.server
	}
	
	pub fn get_client_config(&self) -> &ClientConfig {
		&self.client
	}
}

#[derive(Deserialize)]
pub struct ServerConfig {
	files_root: String
}

impl ServerConfig {
	pub fn get_files_root(&self) -> &str {
		self.files_root.as_str()
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
	dirs: Value
}

impl Mappings {
	pub fn get_file_mappings(&self) -> &Table {
		self.files.as_table().expect("File mappings are not a table")
	}
	
	pub fn get_dir_mappings(&self) -> &Table {
		self.dirs.as_table().expect("Dir mappings are not a table")
	}
}

pub struct Mapping {
	client_path_buf: PathBuf,
	server_path_buf: PathBuf
}

impl Mapping {
	pub fn from_table_entry((client_file_string, server_file_value): (&String, &Value)) -> Self {
		let client_mapping_str = client_file_string.as_str();
		let client_path_buf = Path::new(client_mapping_str).to_path_buf();

		let server_mapping_str = server_file_value.as_str()
			.expect(format!("Mapping value was not a string: {:?}", server_file_value).as_str());
		let server_path_buf = Path::new(server_mapping_str).to_path_buf();

		Self {
			client_path_buf,
			server_path_buf
		}
	}

	pub fn get_client_path(&self) -> &Path {
		&self.client_path_buf
	}

	pub fn get_client_path_str(&self) -> &str {
		&self.client_path_buf.to_str().expect("Client path could not be interpreted as &str")
	}

	pub fn get_server_path(&self) -> &Path {
		&self.server_path_buf
	}

	pub fn get_server_path_str(&self) -> &str {
		&self.server_path_buf.to_str().expect("Server path could not be interpreted as &str")
	}
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct FileInfo {
	path: PathBuf,
	seconds: u64,
	digest: String,
	exists: bool
}

#[derive(Debug)]
pub enum FileInfoError {
	NotFile,
	NotDir,
	NotFound
}

impl FileInfo {
	pub fn from_file_path(path: PathBuf) -> Result<Self, FileInfoError> {
		if !path.is_file() {
			if !path.exists() {
				return Ok(
					Self {
						path,
						seconds: 0,
						digest: "".to_string(),
						exists: false
					}
				);
			}
			
			return Err(FileInfoError::NotFile);
		}

		let seconds = modified_seconds_path(&path);
		let digest = sha256_digest_path(&path);

		Ok(
			Self {
				path,
				seconds,
				digest,
				exists: true
			}
		)
	}

	pub fn from_dir_path(path: &Path) -> Result<Vec<Self>, FileInfoError> {
		if !path.is_dir() {
			if !path.exists() {
				return Ok(Vec::new());
			}
			
			return Err(FileInfoError::NotDir);
		}

		let paths = Self::walk_dir(path).unwrap();

		Ok(paths.into_iter().map(|p| Self::from_file_path(p).unwrap()).collect())
	}

	pub fn get_path(&self) -> &Path {
		&self.path
	}

	pub fn get_seconds(&self) -> u64 {
		self.seconds
	}

	pub fn get_digest(&self) -> &str {
		&self.digest
	}

	pub fn exists(&self) -> bool {
		self.exists
	}

	pub fn strip_prefix<P: AsRef<Path>>(&mut self, prefix: P) -> Result<(), StripPrefixError> {
		self.path = self.path.strip_prefix(prefix)?.to_path_buf();
		Ok(())
	}

	fn walk_dir(path: &Path) -> Result<Vec<PathBuf>, FileInfoError> {
		if !path.exists() {
			return Err(FileInfoError::NotFound);
		}

		if !path.is_dir() {
			return Err(FileInfoError::NotDir);
		}

		let mut paths = Vec::new();

		if let Ok(iter) = fs::read_dir(path) {
			for entry in iter {
				if let Ok(entry) = entry {
					let path = entry.path();
					if path.is_dir() {
						if let Ok(mut subpaths) = Self::walk_dir(path.as_path()) {
							paths.append(&mut subpaths);
						}
					} else {
						paths.push(path)
					}
				}
			}
		}

		Ok(paths)
	}
}

fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest, io::Error> {
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

fn sha256_digest_path(path: &Path) -> String {
	let mut digest_string = "".to_string();
	if let Ok(input_file) = fs::File::open(path) {
		let file_reader = BufReader::new(input_file);
		if let Ok(digest) = sha256_digest(file_reader) {
			digest_string = HEXUPPER.encode(digest.as_ref()).to_string();
		}
	}
	digest_string
}

fn modified_seconds_path(path: &Path) -> u64 {
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
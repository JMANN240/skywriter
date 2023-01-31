use std::ffi::OsStr;
use ring::digest::{Context, Digest, SHA256};
use rocket::Request;
use rocket::request::{FromRequest, Outcome};
use rocket::http::Status;
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

// A structure for representing the config file
#[derive(Deserialize)]
pub struct Config {
	server: ServerConfig,
	client: ClientConfig
}

impl Config {

	// Constructor

	pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
		let config_string = fs::read_to_string(path).unwrap();
		toml::from_str(&config_string).unwrap()
	}

	// Getters

	pub fn get_server_config(&self) -> &ServerConfig {
		&self.server
	}
	
	pub fn get_client_config(&self) -> &ClientConfig {
		&self.client
	}
}

// A structure for representing the server config
#[derive(Deserialize)]
pub struct ServerConfig {
	files_root: String,
	password: String,
	ignored_paths: Value
}

impl ServerConfig {

	// Getters

	pub fn get_files_root(&self) -> &str {
		self.files_root.as_str()
	}

	pub fn get_password(&self) -> &str {
		self.password.as_str()
	}

	pub fn get_ignored_paths(&self) -> Vec<&OsStr> {
		self.ignored_paths.as_array().expect("Ignored paths is not an array").as_slice().iter().map(|p| OsStr::new(p.as_str().expect("Ignored path is not string"))).collect()
	}
}

// A structure for representing the client config
#[derive(Deserialize)]
pub struct ClientConfig {
	server_url: String,
	mappings: Mappings
}

impl ClientConfig {

	// Getters

	pub fn get_mappings(&self) -> &Mappings {
		&self.mappings
	}
	
	pub fn get_server_url(&self) -> &str {
		&self.server_url
	}
}

// A structure for representing the file and directory mappings
#[derive(Deserialize)]
pub struct Mappings {
	files: Value,
	dirs: Value
}

impl Mappings {

	// Getters

	pub fn get_file_mappings(&self) -> &Table {
		self.files.as_table().expect("File mappings are not a table")
	}
	
	pub fn get_dir_mappings(&self) -> &Table {
		self.dirs.as_table().expect("Dir mappings are not a table")
	}
}

// A structure for representing a mapping from a client path to a server path
pub struct Mapping {
	client_path_buf: PathBuf,
	server_path_buf: PathBuf
}

impl Mapping {

	// Constructor

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

	// Getters

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

// A structure for representing the pertinent information of a file
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct FileInfo {
	path: PathBuf, // The path of the file
	seconds: u64, // When it was last modified
	digest: String, // Its SHA-256 digest
	exists: bool // If it exists
}

// Things that can go wrong when making a FileInfo
#[derive(Debug)]
pub enum FileInfoError {
	NotFile,
	NotDir,
	NotFound
}

impl FileInfo {
	// Associated function to make a single FileInfo struct based on a path
	pub fn from_file_path(path: PathBuf) -> Result<Self, FileInfoError> {
		if !path.is_file() {
			if !path.exists() {
				// If the path is not a file and does not exist, return a non-existent FileInfo struct
				return Ok(
					Self {
						path,
						seconds: 0,
						digest: "".to_string(),
						exists: false,
					}
				);
			}
			
			// If the path is not a file but does exist (meaning it is an existing path), return an error
			return Err(FileInfoError::NotFile);
		}

		// Get some info based on the path
		let seconds = modified_seconds_path(&path);
		let digest = sha256_digest_path(&path);

		// Build and return the FileInfo structure
		Ok(
			Self {
				path,
				seconds,
				digest,
				exists: true
			}
		)
	}

	// Associated function to make a vector of FileInfo structs based on a path
	pub fn from_dir_path(path: &Path) -> Result<Vec<Self>, FileInfoError> {
		if !path.is_dir() {
			if !path.exists() {
				// If the path is not a directory and does not exist, return an empty vector
				return Ok(Vec::new());
			}
			
			// If the path is not a directory and does exist (meaning it is an existing file), return an error
			return Err(FileInfoError::NotDir);
		}

		// Get the vector of PathBuf structs under the given path
		let paths = Self::walk_dir(path).unwrap();

		// Turn the PathBufs into FileInfos and return the new vector
		Ok(paths.into_iter().map(|p| Self::from_file_path(p).unwrap()).collect())
	}

	// Getters

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

	// Remove a prefix from the path
	pub fn strip_prefix<P: AsRef<Path>>(&mut self, prefix: P) -> Result<(), StripPrefixError> {
		self.path = self.path.strip_prefix(prefix)?.to_path_buf();
		Ok(())
	}

	// Private utility function to recursively search a directory
	fn walk_dir(path: &Path) -> Result<Vec<PathBuf>, FileInfoError> {
		// If the path doesn't exist, how are we going to walk it?
		if !path.exists() {
			return Err(FileInfoError::NotFound);
		}

		// If the path is not a directory, how are we going to walk it?
		if !path.is_dir() {
			return Err(FileInfoError::NotDir);
		}

		// Create an empty vector
		let mut paths = Vec::new();

		// If reading the given path's directory goes ok
		if let Ok(iter) = fs::read_dir(path) {
			// Loop though each entry
			for entry in iter {
				// If each entry is read ok
				if let Ok(entry) = entry {
					// Get the path of the entry
					let path = entry.path();

					if path.is_dir() { // If it is a directory
						// Walk that directory and append it's returned vector to our own
						if let Ok(mut subpaths) = Self::walk_dir(path.as_path()) {
							paths.append(&mut subpaths);
						}
					} else { // If it is a file
						// Add it to our own vector of PathBufs
						paths.push(path)
					}
				}
			}
		}

		// Return the vector of PathBufs
		Ok(paths)
	}
}

// Utility function to get the SHA-256 digest of a stream of bytes
fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest, io::Error> {
	// Create a new SHA-256 context and buffer
	let mut context = Context::new(&SHA256);
	let mut buffer = [0; 1024];

	// Read from the stream into the buffer, break if nothing was read, and update the SHA-256 algorithm
	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		context.update(&buffer[..count]);
	}

	// Return the finished digest
	Ok(context.finish())
}

// A wrapper around sha256_digest, given a path
fn sha256_digest_path(path: &Path) -> String {
	// Create a string to hold the eventual digest
	let mut digest_string = "".to_string();

	// If opening the file at the given path goes ok
	if let Ok(input_file) = fs::File::open(path) {
		// Create a BufReader on the opened file
		let file_reader = BufReader::new(input_file);

		// If the file was digested ok, assign the digest as upper-case hexadecimal to digest_string
		if let Ok(digest) = sha256_digest(file_reader) {
			digest_string = HEXUPPER.encode(digest.as_ref()).to_string();
		}
	}

	// Return the digest_string
	digest_string
}

// Utility function to get when a path was last modified
fn modified_seconds_path(path: &Path) -> u64 {
	// Create a unsigned 64-bit integer to hold the eventual number of seconds
	let mut modified_seconds = 0;

	// If getting the metadata of the path goes ok
	if let Ok(metadata) = fs::metadata(path) {
		// If getting the modified time from the metadata goes ok
		if let Ok(modified_time) = metadata.modified() {
			// If getting the duration between the unix epoch and the modified time goes ok
			if let Ok(duration) = modified_time.duration_since(SystemTime::UNIX_EPOCH) {
				// Assign the duration as the number of seconds to modified_seconds
				modified_seconds = duration.as_secs();
			}
		}
	}
	
	// Return the modified_seconds
	modified_seconds
}

// A request guard strucure for getting authenticaing a request
pub struct ValidPassword;

// Things that could go wrong with a valid password
#[derive(Debug)]
pub enum PasswordValidationError {
	IncorrectPassword,
	PasswordHeaderMissing
}

// Request guard logic
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ValidPassword {
	type Error = PasswordValidationError;

	async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		// Make sure that the 'password' header is present, fail and report if not
		match req.headers().get_one("password") {
			Some(password) => {
				// Get the actual password from the config file
				let actual_password = req.rocket().state::<Config>().unwrap().get_server_config().get_password();

				// Check if the given password is equal to the actual password
				if password == actual_password {
					return Outcome::Success(Self);
				} else {
					return Outcome::Failure((Status::Unauthorized, PasswordValidationError::IncorrectPassword));
				}
			},
			None => {
				return Outcome::Failure((Status::Unauthorized, PasswordValidationError::PasswordHeaderMissing));
			}
		}
	}
}

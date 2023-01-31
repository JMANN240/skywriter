use std::io;
use std::fs;
use std::path::Path;
use reqwest::StatusCode;
use reqwest::{multipart, Body};
use skywriter::{FileInfo, Config, ClientConfig, ServerConfig, Mappings, Mapping};
use tokio_util::codec::{BytesCodec, FramedRead};
use toml::value::Table;

struct Client {
	config: Config
}

impl Client {
	pub fn new() -> Self {
		Self {
			config: Config::from_file("Config.toml")
		}
	}

	fn get_config(&self) -> &Config {
		&self.config
	}
	
	fn get_client_config(&self) -> &ClientConfig {
		&self.get_config().get_client_config()
	}
	
	fn get_server_config(&self) -> &ServerConfig {
		&self.get_config().get_server_config()
	}
	
	fn get_password(&self) -> &str {
		self.get_server_config().get_password()
	}
	
	fn get_server_url(&self) -> &str {
		&self.get_client_config().get_server_url()
	}
	
	fn get_mappings(&self) -> &Mappings {
		&self.get_client_config().get_mappings()
	}

	pub fn get_file_mappings(&self) -> &Table {
		&self.get_mappings().get_file_mappings()
	}

	pub fn get_dir_mappings(&self) -> &Table {
		&self.get_mappings().get_dir_mappings()
	}

	// Synchronize all mapped files
	pub async fn sync_files(&self, file_mappings: &Table) -> () {
		// Go through all of the file mappings and update the files
		for mapping in file_mappings.iter().map(Mapping::from_table_entry) {
			self.update_file(mapping.get_client_path(), mapping.get_server_path()).await;
		}
	}

	// Synchronize all mapped directories
	pub async fn sync_dirs(&self, dir_mappings: &Table) -> () {
		// Go through all of the directory mappings and update the directories
		for mapping in dir_mappings.iter().map(Mapping::from_table_entry) {
			self.update_dir(mapping.get_client_path(), mapping.get_server_path()).await;
		}
	}

	// Update a file on the client or server based on which is most recent
	async fn update_file(&self, client_file_path: &Path, server_file_path: &Path) -> () {
		// Get the file info on the client, panic if unable to build the FileInfo struct
		let client_file_info = FileInfo::from_file_path(client_file_path.to_path_buf())
			.expect("Could not build FileInfo for client path");

		// Try to parse the configured server path for the mapping as a &str, panic if unable
		let server_file_path_str = server_file_path.to_str()
			.expect("Server file path could not be interpreted as &str");

		// Create the HTTP client and ask the server for the file information
		let client = reqwest::Client::new();
		let res_result = client
			.get(format!("{}/info/file{}", self.get_server_url(), server_file_path_str))
			.header("password", self.get_password())
			.send()
			.await;

		// If everything went ok, get the response, otherwise return
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not get file info, status {:?}", e.status());
				return;
			}
		};

		// If not authorized, report and return
		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not get file info, status {:?}", res.status());
			return;
		}

		// Build the FileInfo struct for the file on the server, panic if unable
		let server_file_info = res.json::<FileInfo>().await
			.expect("Could not build FileInfo for server path");
		
		// Here is the real logic of syncing the files comes in
		match (client_file_info.exists(), server_file_info.exists()) {
			// If the file exists on the client and the server, do some more checks
			(true, true) => {
				// If the server and client do not have the same file contents (meaning there has been a change since the last sync)
				if client_file_info.get_digest() != server_file_info.get_digest() {
					// Depending on which was more recently changed, upload or download
					if client_file_info.get_seconds() < server_file_info.get_seconds() {
						self.download(server_file_info.get_path(), client_file_info.get_path()).await;
					} else {
						self.upload(client_file_info.get_path(), server_file_info.get_path()).await;
					}
				}
			},
			// If the file exists on the client but not on the server, upload it
			(true, false) => {
				self.upload(client_file_info.get_path(), server_file_info.get_path()).await;
			},
			// If the file exists on the server but not on the client, download it
			(false, true) => {
				self.download(server_file_info.get_path(), client_file_info.get_path()).await;
			},
			// If the file does not exist anywhere, do nothing
			(false, false) => {}
		}
	}

	// Update the files in a directory on the client or server based on which are most recent
	async fn update_dir(&self, client_dir_path: &Path, server_dir_path: &Path) -> () {
		// Get the file infos on the client, panic if unable to build the FileInfo structs
		let client_file_infos = FileInfo::from_dir_path(client_dir_path)
			.expect(format!("Could not build FileInfo for client path {:?}", client_dir_path).as_str());
		
		// Try to parse the configured server path for the mapping as a &str, panic if unable
		let server_dir_path_str = server_dir_path.to_str().expect("Server dir path could not be interpreted as &str");

		// Create the HTTP client and ask the server for the directory information
		let client = reqwest::Client::new();
		let res_result = client
			.get(format!("{}/info/dir{}", self.get_server_url(), server_dir_path_str))
			.header("password", self.get_password())
			.send()
			.await;
		
		// If everything went ok, get the response, otherwise return
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not get dir info, status {:?}", e.status());
				return;
			}
		};

		// If not authorized, report and return
		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not get dir info, status {:?}", res.status());
			return;
		}

		// Build the FileInfo structs for the files in the directory on the server, panic if unable
		let server_file_infos = res.json::<Vec<FileInfo>>().await
			.expect(format!("Could not build FileInfos for server path {:?}", server_dir_path).as_str());
		
		// Loop through each file on the client
		for file_info in client_file_infos.iter() {
			// Get the path to the file relative to the client's directory's path, panic if unable
			let client_file_path = file_info.get_path();
			let dir_file_path = client_file_path.strip_prefix(client_dir_path)
				.expect("Could not strip client dir path prefix");
			// Build the path of the file on the server
			let mut server_file_path = server_dir_path.to_path_buf();
			server_file_path.push(dir_file_path);
			// Update the file, syncing based on which is most recent
			self.update_file(client_file_path, &server_file_path).await;
		}
		
		// Loop through each file on the server
		for file_info in server_file_infos.iter() {
			// Get the path to the file relative to the server's directory's path
			let mut server_file_path = server_dir_path.to_path_buf();
			server_file_path.push(file_info.get_path());
			// Build the path of the file on the client
			let mut client_file_path = client_dir_path.to_path_buf();
			client_file_path.push(file_info.get_path());
			// Update the file, syncing based on which is most recent
			self.update_file(&client_file_path, &server_file_path).await;
		}
	}
	
	// Download a file located at server_path from the server and save it to client_path
	async fn download(&self, server_path: &Path, client_path: &Path) -> () {
		// Get the directory that the file will be saved to, create it if it doesn't exist, panic if it has no parent
		let parent_path = client_path.parent()
			.expect(format!("Client path {:?} has no parent", client_path).as_str());
		fs::create_dir_all(parent_path).expect(format!("Could not create dirs needed for {:?}", client_path).as_str());

		// Create the file that will be written to
		let mut file = fs::File::create(client_path)
			.expect("File creation failed");
		
		// Try to parse the given server path as a &str, panic if unable
		let server_path = server_path.to_str()
			.expect("Server path could not be interpreted as &str");
		
		// Create the HTTP client and download the file from the server
		let client = reqwest::Client::new();
		let res_result = client
			.get(format!("{}/file/{}", self.get_server_url(), server_path))
			.header("password", self.get_password())
			.send()
			.await;

		// If everything went ok, get the response, otherwise return
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not download file, status {:?}", e.status());
				return;
			}
		};

		// If not authorized, report and return
		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not download file, status {:?}", res.status());
			return;
		}

		// Get the response text and copy it as bytes to the created file
		let res_text = res.text().await
			.expect("Text extraction failed");
		io::copy(&mut res_text.as_bytes(), &mut file).expect("Copy failed");

	}
	
	// Upload a file located at client_path from the client and save it to server_path on the server
	async fn upload(&self, client_path: &Path, server_path: &Path) -> () {
		// Try to parse the given server path as a &str, panic if unable
		let server_path = server_path.to_str().expect("Server path could not be interpreted as &str");

		// Create the HTTP client and the object for the file to be uploaded
		let client = reqwest::Client::new();
		let file = tokio::fs::File::open(client_path).await.unwrap();
		let stream = FramedRead::new(file, BytesCodec::new());
		let stream_body = Body::wrap_stream(stream);
		let upload_stream = multipart::Part::stream(stream_body);
		let form = multipart::Form::new().part("file", upload_stream);

		// Upload the file using a put request
		let res_result = client
			.put(format!("{}/file/{}", self.get_server_url(), server_path))
			.multipart(form)
			.header("password", self.get_password())
			.send().await;
		
		// If everything went ok, get the response, otherwise return
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not upload file, status {:?}", e.status());
				return;
			}
		};

		// If not authorized, report and return
		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not upload file, status {:?}", res.status());
			return;
		}
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = Client::new();
	println!("1");
	client.sync_files(client.get_file_mappings()).await;
	println!("2");
	client.sync_dirs(client.get_dir_mappings()).await;
	println!("1");
	println!("1");
	println!("3");
	Ok(())
}

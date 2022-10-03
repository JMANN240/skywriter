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

	pub async fn sync_files(&self, file_mappings: &Table) -> () {
		for mapping in file_mappings.iter().map(Mapping::from_table_entry) {
			self.update_file(mapping.get_client_path(), &mapping.get_server_path()).await;
		}
	}

	pub async fn sync_dirs(&self, dir_mappings: &Table) -> () {
		for mapping in dir_mappings.iter().map(Mapping::from_table_entry) {
			self.update_dir(mapping.get_client_path(), mapping.get_server_path()).await;
		}
	}

	async fn update_file(&self, client_file_path: &Path, server_file_path: &Path) -> () {
		let client_file_info = FileInfo::from_file_path(client_file_path.to_path_buf())
			.expect("Could not build FileInfo for client path");

		let server_file_path_str = server_file_path.to_str().expect("Server file path could not be interpreted as &str");

		let client = reqwest::Client::new();
		let res_result = client.get(format!("{}/info/file{}", self.get_server_url(), server_file_path_str)).header("password", self.get_password()).send().await;

		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not get file info, status {:?}", e.status());
				return;
			}
		};

		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not get file info, status {:?}", res.status());
			return;
		}

		let server_file_info = res.json::<FileInfo>().await
			.expect("Could not build FileInfo for server path");
		
		match (client_file_info.exists(), server_file_info.exists()) {
			(true, true) => {
				if client_file_info.get_digest() != server_file_info.get_digest() {
					if client_file_info.get_seconds() < server_file_info.get_seconds() {
						self.download(server_file_info.get_path(), client_file_info.get_path()).await;
					} else {
						self.upload(client_file_info.get_path(), server_file_info.get_path()).await;
					}
				}
			},
			(true, false) => {
				self.upload(client_file_info.get_path(), server_file_info.get_path()).await;
			},
			(false, true) => {
				self.download(server_file_info.get_path(), client_file_info.get_path()).await;
			},
			(false, false) => {}
		}
	}

	async fn update_dir(&self, client_dir_path: &Path, server_dir_path: &Path) -> () {
		let client_file_infos = FileInfo::from_dir_path(client_dir_path)
			.expect(format!("Could not build FileInfo for client path {:?}", client_dir_path).as_str());
		
		let server_dir_path_str = server_dir_path.to_str().expect("Server dir path could not be interpreted as &str");

		let client = reqwest::Client::new();
		let res_result = client.get(format!("{}/info/dir{}", self.get_server_url(), server_dir_path_str)).header("password", self.get_password()).send().await;
		
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not get dir info, status {:?}", e.status());
				return;
			}
		};

		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not get dir info, status {:?}", res.status());
			return;
		}

		let server_file_infos = res.json::<Vec<FileInfo>>().await
			.expect(format!("Could not build FileInfo for server path {:?}", server_dir_path).as_str());
		
		for file_info in client_file_infos.iter() {
			let client_file_path = file_info.get_path();
			let dir_file_path = client_file_path.strip_prefix(client_dir_path)
				.expect("Could not strip client dir path prefix");
			let mut server_file_path = server_dir_path.to_path_buf();
			server_file_path.push(dir_file_path);
			self.update_file(client_file_path, &server_file_path).await;
		}
		
		for file_info in server_file_infos.iter() {
			let mut server_file_path = server_dir_path.to_path_buf();
			server_file_path.push(file_info.get_path());
			let mut client_file_path = client_dir_path.to_path_buf();
			client_file_path.push(file_info.get_path());
			self.update_file(&client_file_path, &server_file_path).await;
		}
	}
	
	async fn download(&self, server_path: &Path, client_path: &Path) -> () {
		let parent_path = client_path.parent().expect(format!("Client path {:?} has no parent", client_path).as_str());
		fs::create_dir_all(parent_path).expect(format!("Could not create dirs needed for {:?}", client_path).as_str());
		let mut file = fs::File::create(client_path).expect("File creation failed");
		
		let server_path = server_path.to_str().expect("Server path could not be interpreted as &str");
		let client = reqwest::Client::new();
		let res_result = client.get(format!("{}/file/{}", self.get_server_url(), server_path)).header("password", self.get_password()).send().await;

		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not download file, status {:?}", e.status());
				return;
			}
		};

		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not download file, status {:?}", res.status());
			return;
		}

		let res_text = res.text().await.expect("Text extraction failed");
		io::copy(&mut res_text.as_bytes(), &mut file).expect("Copy failed");

	}
	
	async fn upload(&self, client_path: &Path, server_path: &Path) -> () {
		let server_path = server_path.to_str().expect("Server path could not be interpreted as &str");
		let client = reqwest::Client::new();
		let file = tokio::fs::File::open(client_path).await.unwrap();
		let stream = FramedRead::new(file, BytesCodec::new());
		let stream_body = Body::wrap_stream(stream);
		let upload_stream = multipart::Part::stream(stream_body);
		let form = multipart::Form::new().part("file", upload_stream);

		let res_result = client.put(format!("{}/file/{}", self.get_server_url(), server_path))
			.multipart(form)
			.header("password", self.get_password())
			.send().await;
		
		let res = match res_result {
			Ok(res) => res,
			Err(e) => {
				println!("Could not upload file, status {:?}", e.status());
				return;
			}
		};

		if res.status() == StatusCode::UNAUTHORIZED {
			println!("Could not upload file, status {:?}", res.status());
			return;
		}
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = Client::new();
	client.sync_files(client.get_file_mappings()).await;
	client.sync_dirs(client.get_dir_mappings()).await;
	Ok(())
}
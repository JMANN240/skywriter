use reqwest;
use std::io::Read;
use skywriter::{parse_config_file, Config};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;

struct SkywriterClient {
	config: Config
}

impl SkywriterClient {
	fn new() -> Self {
		SkywriterClient {
			config: parse_config_file()
		}
	}

	fn download_files(&self) {
		let client_config = self.config.get_client_config();
		let mappings = client_config.get_mappings();
		let file_mappings = mappings.get_file_mappings();
		for (client_path, server_path_value) in file_mappings.iter() {
			if let Some(server_path) = server_path_value.as_str() {
				self.download_file(Path::new(client_path).to_path_buf(), Path::new(server_path).to_path_buf())
			}
		}
	}
	
	fn download_file(&self, client_file_path: PathBuf, server_file_path: PathBuf) {
		let client_config = self.config.get_client_config();
		let server_url = client_config.get_server_url();
		if let Ok(mut res) = reqwest::blocking::get(format!("{}/file{}", server_url, server_file_path.as_path().display())) {
			if let Ok(mut client_file) = fs::File::create(client_file_path.as_path()) {
				if let Ok(content) = res.text() {
					client_file.write(content.as_bytes());
				}
			}
		}
	}
}

fn main() {
	let client = SkywriterClient::new();
	client.download_files();
}
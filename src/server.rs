#[macro_use] extern crate rocket;
use rocket::fs::{NamedFile, TempFile};
use rocket::http::uri::Segments;
use rocket::http::Status;
use rocket::form::Form;
use rocket::State;
use std::path::{Path, PathBuf};
use std::vec;
use rocket::serde::json::Json;
use std::fs;

use skywriter::{FileInfo, Config, ValidPassword};

// Health check route
#[get("/")]
async fn index() -> &'static str {
    "Skywriter Operational"
}

// Route for getting a file
#[get("/file/<virtual_path_segments..>")]
async fn get_file(virtual_path_segments: Segments<'_, rocket::http::uri::fmt::Path>, config: &State<Config>, _password: ValidPassword) -> Result<NamedFile, Status> {
    // Turn the segments into PathBuf
    let virtual_path = virtual_path_segments.to_path_buf(true).unwrap();

	// Get the full path for the file based on the configured file root
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);

	// Check to see if the given path could create a FileInfo struct, return 422 otherwise
	match FileInfo::from_file_path(full_path) {
		Ok(file_info) => {
			// If the file exists, return it, otherwise return 404
			let ignored = config.get_server_config().get_ignored_paths().contains(&file_info.get_path().as_os_str());
			if file_info.exists() && !ignored {
				Ok(NamedFile::open(file_info.get_path()).await.unwrap())
			} else {
				Err(Status::NotFound)
			}
		},
		Err(_) => {
			Err(Status::UnprocessableEntity)
		}
	}
}

// Structure for getting the uploaded file
#[derive(FromForm)]
pub struct FileUpload<'r> {
	file: TempFile<'r>
}

impl<'r> FileUpload<'r> {
	// Move the file out of the struct
	pub fn take_file(self) -> TempFile<'r> {
		self.file
	}
}

// Route for uploading a file
#[put("/file/<virtual_path_segments..>", data="<form>")]
async fn put_file(virtual_path_segments: Segments<'_, rocket::http::uri::fmt::Path>, form: Form<FileUpload<'_>>, config: &State<Config>, _password: ValidPassword) -> Status {
    // Turn the segments into PathBuf
    let virtual_path = virtual_path_segments.to_path_buf(true).unwrap();

	// Get the full path for the file based on the configured file root
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);

	// Check to see if we should ignore it
	let ignored = config.get_server_config().get_ignored_paths().contains(&full_path.as_path().as_os_str());
	if ignored {
		return Status::NoContent;
	}

	// Check to see if the given path could have a parent directory, return 422 otherwise
	match full_path.parent() {
		Some(parent_path) => {
			// Try to create that parent path if it doesn't exist, return 403 otherwise
			match fs::create_dir_all(parent_path) {
				Ok(()) => {
					// Try to get the uploaded file and save it to full_path, return 500 otherwise
					match form.into_inner().take_file().persist_to(full_path).await {
						Ok(()) => {
							Status::Created
						},
						Err(_) => {
							Status::InternalServerError
						}
					}
				},
				Err(_) => {
					Status::Forbidden
				}
			}
		},
		None => {
			Status::UnprocessableEntity
		}
	}
}

// Route for getting a file's information
#[get("/info/file/<virtual_path_segments..>")]
async fn get_file_info(virtual_path_segments: Segments<'_, rocket::http::uri::fmt::Path>, config: &State<Config>, _password: ValidPassword) -> Result<Json<FileInfo>, Status> {
    // Turn the segments into PathBuf
    let virtual_path = virtual_path_segments.to_path_buf(true).unwrap();

	// Get the full path for the file based on the configured file root
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);

	// Check to see if the given path could create a FileInfo struct, return 422 otherwise
	match FileInfo::from_file_path(full_path) {
		Ok(mut file_info) => {
			// Strip the server's file root prefix from file_info and return it as JSON
			file_info.strip_prefix(config.get_server_config().get_files_root()).unwrap();
			Ok(Json(file_info))
		},
		Err(_) => {
			Err(Status::UnprocessableEntity)
		}
	}
}

#[get("/info/dir/<virtual_path_segments..>")]
async fn get_dir_info(virtual_path_segments: Segments<'_, rocket::http::uri::fmt::Path>, config: &State<Config>, _password: ValidPassword) -> Result<Json<Vec<FileInfo>>, Status> {
    // Turn the segments into PathBuf
    let virtual_path = virtual_path_segments.to_path_buf(true).unwrap();

	// Get the full path for the file based on the configured file root
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);

	// Check to see if the given path could create a vector of FileInfo structs, return 422 otherwise
	match FileInfo::from_dir_path(full_path.as_path()) {
		Ok(mut file_infos) => {
			// Strip the server's file root prefix from file_infos and return it as JSON
			file_infos.iter_mut().for_each(|fi| fi.strip_prefix(&full_path).unwrap());
			Ok(Json(file_infos))
		},
		Err(_) => {
			Err(Status::UnprocessableEntity)
		}
	}
}

#[launch]
fn rocket() -> _ {
	let config = Config::from_file("Config.toml");
	if config.get_server_config().get_password() == "testpass" {
		println!();
		println!("DEFAULT PASSWORD DETECTED!");
		println!("Make sure you change the password from the default.");
		println!();
	}
	rocket::build()
		.manage(config)
		.mount("/", routes![index, get_file, put_file, get_file_info, get_dir_info])
}

#[macro_use] extern crate rocket;
use rocket::fs::{NamedFile, TempFile};
use rocket::http::Status;
use rocket::form::Form;
use rocket::State;
use std::path::{Path, PathBuf};
use std::vec;
use rocket::serde::json::Json;
use std::fs;

use skywriter::{FileInfo, Config};

#[get("/file/<virtual_path..>")]
async fn get_file(virtual_path: PathBuf, config: &State<Config>) -> Result<NamedFile, Status> {
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);
	match FileInfo::from_file_path(full_path) {
		Ok(file_info) => {
			if file_info.exists() {
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

#[derive(FromForm)]
pub struct FileUpload<'r> {
	file: TempFile<'r>
}

impl<'r> FileUpload<'r> {
	pub fn take_file(self) -> TempFile<'r> {
		self.file
	}
}

#[put("/file/<virtual_path..>", data="<form>")]
async fn put_file(virtual_path: PathBuf, form: Form<FileUpload<'_>>, config: &State<Config>) -> Status {
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);
	match full_path.parent() {
		Some(parent_path) => {
			match fs::create_dir_all(parent_path) {
				Ok(()) => {
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

#[get("/info/file/<virtual_path..>")]
async fn get_file_info(virtual_path: PathBuf, config: &State<Config>) -> Result<Json<FileInfo>, Status> {
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);
	match FileInfo::from_file_path(full_path) {
		Ok(mut file_info) => {
			file_info.strip_prefix(config.get_server_config().get_files_root()).unwrap();
			Ok(Json(file_info))
		},
		Err(_) => {
			Err(Status::UnprocessableEntity)
		}
	}
}

#[get("/info/dir/<virtual_path..>")]
async fn get_dir_info(virtual_path: PathBuf, config: &State<Config>) -> Result<Json<Vec<FileInfo>>, Status> {
	let full_path = Path::new(config.get_server_config().get_files_root()).join(virtual_path);
	match FileInfo::from_dir_path(full_path.as_path()) {
		Ok(mut file_infos) => {
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
	rocket::build()
	.manage(Config::from_file("Config.toml"))
		.mount("/", routes![get_file, put_file, get_file_info, get_dir_info])
}

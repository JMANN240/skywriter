#[macro_use] extern crate rocket;
use rocket::http::Status;
use rocket::fs::NamedFile;
use rocket::form::Form;
use rocket::{State, Response};
use std::path::{Path, PathBuf};
use std::fs;
use std::vec;
use rocket::serde::json::Json;

use skywriter::{Config, parse_config_file, ExistingFile, FileUpload, walk_dir, FileHeaders};

#[get("/")]
fn index() -> Status {
	Status::Ok
}

#[get("/files/<virtual_path..>")]
async fn get_files(virtual_path: PathBuf, config: &State<Config>) -> Json<Vec<FileHeaders>> {
	let file_root = Path::new(config.get_server_config().get_file_root()).join(virtual_path);
	let paths = walk_dir(file_root.as_path());
	let file_headers = paths.into_iter().map(|path| FileHeaders::from_path_buf(path)).collect();
	Json(file_headers)
}

#[get("/file/<_virtual_path..>")]
async fn get_file(_virtual_path: PathBuf, file: ExistingFile) -> NamedFile {
	file.take_file()
}

#[put("/file/<virtual_path..>", data="<form>")]
async fn put_file(config: &State<Config>, form: Form<FileUpload<'_>>, virtual_path: PathBuf) -> Status {
	let full_path = Path::new(config.get_server_config().get_file_root()).join(virtual_path);
	match full_path.parent() {
		Some(directory_path) => {
			match fs::create_dir_all(directory_path) {
				Ok(()) => {
					match form.into_inner().take_file().persist_to(full_path).await {
						Ok(()) => {
							return Status::Created;
						},
						Err(_) => {
							return Status::InternalServerError;
						}
					}
				},
				Err(_) => {
					return Status::UnprocessableEntity;
				}
			}
		},
		None => {
			return Status::UnprocessableEntity;
		}
	}
}

#[get("/info/<virtual_path..>")]
async fn get_info(config: &State<Config>, virtual_path: PathBuf) -> Json<FileHeaders> {
	let full_path = Path::new(config.get_server_config().get_file_root()).join(virtual_path);
	Json(FileHeaders::from_path_buf(full_path))
}

#[launch]
fn rocket() -> _ {
	let config = parse_config_file();
	rocket::build()
		.manage(config)
		.mount("/", routes![index, get_files, get_file, put_file, get_info])
}

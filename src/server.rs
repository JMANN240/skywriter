#[macro_use] extern crate rocket;
use rocket::http::Status;
use std::path::{Path, PathBuf};
use std::vec;
use rocket::serde::json::Json;

use skywriter::{FileInfo};

#[get("/info/file/<virtual_path..>")]
async fn get_file_info(virtual_path: PathBuf) -> Result<Json<FileInfo>, Status> {
	let full_path = Path::new("files").join(virtual_path);
	match FileInfo::from_file_path(full_path) {
		Ok(mut file_info) => {
			file_info.strip_prefix("files").unwrap();
			Ok(Json(file_info))
		},
		Err(_) => {
			Err(Status::UnprocessableEntity)
		}
	}
}

#[get("/info/dir/<virtual_path..>")]
async fn get_dir_info(virtual_path: PathBuf) -> Result<Json<Vec<FileInfo>>, Status> {
	let full_path = Path::new("files").join(virtual_path);
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
		.mount("/", routes![get_file_info, get_dir_info])
}

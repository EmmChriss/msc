extern crate clap;
extern crate serde;
extern crate serde_json;
extern crate toml;
extern crate anyhow;

mod opts;
mod db;

use opts::{Opts, get_opts};
use db::*;

fn main() {
	//let opts = get_opts();
	let mut path = std::env::current_dir().unwrap();
	path.push("mscdb");
	let db = load_db(path).unwrap();
	println!("{:#?}", db);
}

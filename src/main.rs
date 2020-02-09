extern crate clap;
extern crate serde;
extern crate serde_json;

use std::path::{Path, PathBuf};
use clap::{App, Arg, SubCommand, AppSettings};

#[derive(Debug)]
struct Opts {
	pub verbose: bool,
	pub cmd: Cmd
}

#[derive(Debug)]
enum Cmd {
	New {
		path: PathBuf
	},
	List {
		expr: String
	},
	Add {
		url: String,
		playlist: bool
	},
	Remove {
		expr: String
	}
}

fn get_opts() -> Opts {
	// validate paths
	fn is_dir(s: String) -> Result<(), String> {
		let path = Path::new(&s);
		if path.is_dir() {
			Ok(())
		} else if path.exists() {
			Err(format!("{} is not a directory", s))
		} else {
			Err(format!("{}: no such file or directory", s))
		}
	}
	
	let app = App::new("msc - music library handler and downloader")
		.author("EmmChriss <emmchris@protonmail.com>")
		.setting(AppSettings::ArgRequiredElseHelp)
		.global_settings(&[
			AppSettings::VersionlessSubcommands,
			AppSettings::DeriveDisplayOrder,
			AppSettings::DisableHelpSubcommand
		]).arg(Arg::with_name("verbose")
			.short("v").long("verbose")
			.help("Prints more info")
			.global(true))
		.subcommands(vec![
		SubCommand::with_name("new")
			.about("create new library")
			.arg(Arg::with_name("path")
				.help("path at which to create the library [default: pwd]")
				.required(false)
				.validator(is_dir)
			),
		SubCommand::with_name("list")
			.about("list library contents")
			.visible_alias("ls")
			.arg(Arg::with_name("expr")
				.help("expression to search for")
			),
		SubCommand::with_name("add")
			.setting(AppSettings::ArgRequiredElseHelp)
			.about("add to library")
			.visible_alias("a")
			.arg(Arg::with_name("url")
				.help("url to add to the library")
				.required(true))
			.arg(Arg::with_name("playlist")
				.short("p").long("playlist")
				.help("url contains a playlist")
			),
		SubCommand::with_name("remove")
			.setting(AppSettings::ArgRequiredElseHelp)
			.about("remove from library")
			.visible_alias("rm"),
		]);
	let args = app.clone().get_matches();
	
	
	use Cmd::*;
	let opts = Opts {
		verbose: args.is_present("verbose"),
		cmd: {
			let (subcmd, args) = args.subcommand();
			let args = args.unwrap();
			match subcmd {
				"new" => New {
					path: if let Some(s) = args.value_of("path") {
						s.into()
					} else {
						std::env::current_dir().unwrap()
					}
				},
				"list" => List {
					expr: args.value_of("expr").unwrap().to_string()
				},
				"add" => Add {
					playlist: args.is_present("playlist"),
					url: args.value_of("url").unwrap().to_string()
				},
				"remove" => Remove {
					expr: args.value_of("expr").unwrap().to_string()
				},
				_ => unreachable!()
			}
		}
	};
	opts
}

struct Entry {
	pub url: String,
	pub id: String,
	pub title: String,
	pub kind: EntryKind,
	pub cache: Vec<Song>
}

enum EntryKind {
	Single,
	Playlist
}

struct Song {
	pub id: String,
	pub title: String
}

fn main() {
	let opts = get_opts();
	
	
}

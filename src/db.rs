use std::collections::HashMap;
use std::io::{BufReader, BufRead, BufWriter, Lines};
use std::fs::File;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use serde_json::{Value, map::Map};
use anyhow::{Result, Error};

pub struct Cache(Vec<(String, Vec<CacheEntry>)>);

impl Cache {
	pub fn load(path: impl AsRef<Path>) -> Result<Self> {
		let mut cache = Cache(vec![]);
		let err = || Error::msg("Malformed JSON");
		
		let file = File::open(path)?;
		let mut reader = BufReader::new(file);
		
		let val: Value = serde_json::from_reader(reader)?;
		let array = val.as_array().ok_or(err())?;
		for val in array {
			let val     = val.as_object().ok_or(err())?;
			let path    = val.get("path").ok_or(err())?;
			let entries = val.get("entries").ok_or(err())?;
			
			let mut c_path = (path.to_string(), vec![]);
			let entries = entries.as_array().ok_or(err())?;
			for entry in entries {
				let entry: CacheEntry = serde_json::from_value(entry.clone())?;
				c_path.1.push(entry);
			}
			cache.0.push(c_path);
		}
		Ok(cache)
	}
	
	pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
		let json = self.0.iter().map(|(path, entries)| {
			let mut map = Map::new();
			let entries: Vec<_> = entries.iter()
				.map(|entry| serde_json::to_value(entry))
				// discard any error, but that would be useful
				.filter_map(|res| res.ok())
				.collect();
			map.insert("path".to_string(), Value::String(path.clone()));
			map.insert("entries".to_string(), Value::Array(entries));
			Value::Object(map)
		}).collect();
		let json = Value::Array(json);
		
		let file = File::create(path)?;
		let mut writer = BufWriter::new(file);
		serde_json::to_writer(writer, &json).map_err(|e| e.into())
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CacheEntry {
	pub url:   String,
	pub id:    String,
	pub title: String,
}

#[derive(Debug)]
pub struct Db {
	entries: HashMap<PathBuf, Vec<DbEntry>>,
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub struct DbEntry {
	url:   String,
	rules: Vec<DbRule>
}

impl DbEntry {
	pub fn new(url: String, rules: Vec<DbRule>) -> Self {
		DbEntry {
			url,
			rules
		}
	}
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub enum DbRule {
	Opt(String),
	Script(Script)
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub struct Script {
	var: Vec<String>,
	val: Val
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub enum Val {
	Var(Vec<String>),
	String(String),
	Call(String, Vec<Val>)
}

pub fn load_db(path: impl AsRef<Path>) -> Result<Db> {
	let file = File::open(&path)?;
	let mut read = BufReader::new(file);
	
	let mut entries = HashMap::new();
	let mut path = (PathBuf::new(), vec![]);
	let mut rules = vec![];
	
	for line in read.lines() {
		// forward reading error
		let line = line.map(|e| e)?;
		// escape comment
		let line = line.split('#').next().unwrap().to_string();
		let first_char = {
			if let Some(c) = line.chars().next() {
				c
			} else {
				continue;
			}
		};
		match first_char {
			'/' => {
				path.1.push(DbEntry::new(String::new(), rules));
				entries.insert(path.0, path.1);
				rules = vec![];
				
				let path_current = Path::new(&line).to_owned();
				path = (path_current, vec![]);
			},
			'-' => {
				let rule = DbRule::Opt(line);
				rules.push(rule);
			},
			'$' => {
				let script = parse_script(&line);
				rules.push(DbRule::Script(script));
			},
			_ => {
				let mut words = line.split(' ');
				let url = words.next().unwrap().to_string();
				let rules = vec![];
				// TODO: extract rules from url line
				let entry = DbEntry::new(url, rules);
				path.1.push(entry);
			}
		}
	};
	path.1.push(DbEntry::new(String::new(), rules));
	entries.insert(path.0, path.1);
	
	Ok(Db {
		entries
	})
}

fn parse_script(mut script: &str) -> Script {
	script = &script[1..].trim_start();
	let mut split = script.split('=');
	let var: Vec<_> = split.next().unwrap().split('.').map(|s| s.trim().to_string()).collect();
	let mut val = split.next().unwrap();
	
	let (val, _) = parse_val(val.trim_start());
	
	Script {
		var,
		val
	}
}

fn parse_val(mut val: &str) -> (Val, &str) {
	val = val.trim_start();
	if val.chars().next().unwrap() == '"' || val.chars().next().unwrap() == '\'' {
		// parse string
		let closer = val.chars().next().unwrap();
		let idx = val[1..].find(closer).unwrap();
		let val_str = String::from(&val[1..idx+1]);
		val = &val[idx+2..];
		(Val::String(val_str), val)
	} else {
		// parse variables and fn calls
		let idx = val.find(|c: char| !c.is_alphabetic() && c != '_' && c != '.').unwrap();
		let mut id: Vec<String> = val[..idx].split('.').map(|s| s.to_string()).collect();
		let mut val_id = &val[..idx];
		val = &val[idx..];
		if val.chars().next().unwrap() == '(' {
			// fn call
			let mut args = vec![];
			let mut val = &val[1..];
			while val.chars().next().unwrap() != ')' {
				if val.chars().next().unwrap() == ',' {
					val = &val[1..];
				}
				val = val.trim();
				let _val = parse_val(val);
				args.push(_val.0);
				val = _val.1;
			}
			(Val::Call(id[0].clone(), args), val)
		} else {
			// var
			(Val::Var(id), val)
		}
	}
}

pub fn exec_script(script: &Script, ctx: &mut Value) {
	let val_str = exec_val(&script.val, ctx);
	
	let mut var_path = script.var.clone();
	let mut var = ctx;
	while var_path.len() > 1 {
		let path_segm = var_path.pop().unwrap();
		if let Some(var_ch) = var.get_mut(path_segm) {
			if var_ch.as_object_mut().is_some() {
				var = var_ch;
			} else {
				// var_ch not an object, not sure what to do now
				// guess I'll just remake it as an object
				// TODO
			}
		} else {
			let var_ch = Value::Object(Map::new());
			var.as_object_mut().unwrap().insert(path_segm, var_ch);
			var = var.get_mut(path_segm).unwrap();
		}
	}
	let path_last = var_path.pop().unwrap();
	var.as_object_mut().unwrap().insert(path_last, Value::String(val_str));
}

pub fn exec_val(val: &Val, ctx: &Value) -> String {
	match val {
		Val::String(str) => str.clone(),
		Val::Var(path) => {
			let mut path = path;
			let mut var = ctx;
			while let Some(path_segment) = path.pop() {
				if let Some(var_ch) = var.get(path_segment) {
					var = var_ch;
				} else {
					// path non-existent
					return String::new();
				}
			}
			var.as_str().unwrap().to_string()
		},
		Val::Call(name, args) => exec_fn(&name, &args, ctx)
	}
}

pub fn exec_fn(name: &str, args: &Vec<Val>, ctx: &Value) -> String {
	match name {
		"replace" => {
			if args.len() != 3 {
				// ERR
				return String::new();
			}
			
			if let Val::String(_) = args[1] {
				let args: Vec<_> = args.iter().map(|val| exec_val(val, ctx)).collect();
				let str = args[0];
				let from = args[1];
				let to   = args[2];
				return str.replace(&from, &to);
			} else {
				// TODO regex parsing and shit
				unimplemented!();
			}
		},
		_ => {
			// ERR
			// TODO
			return String::new();
		}
	}
}

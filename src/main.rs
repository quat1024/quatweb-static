use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use ramhorns::Ramhorns;
use ramhorns::Content;

use crate::post_db::Post;
use crate::post_db::PostDb;

mod date;
mod markdown;
mod post_db;

fn main() -> Result<()> {
	let pwd = env::current_dir()?;

	let in_dir = pwd.join("in");
	let out_dir = pwd.join("out");

	eprintln!("In: {}\nOut: {}", in_dir.display(), out_dir.display());
	
	//Delete output dir...
	//actually don't, i'm too afraid of deleting something important lol
	
	//Copy static resources
	copy_static(&in_dir.join("static"), &out_dir.join("static")).context("copying static resources")?;

	//Build template engine
	eprintln!("Building template engine");
	let ramhorns = Ramhorns::from_folder(in_dir.join("templates")).context("initializing Ramhorns")?;
	
	//Build post database
	let post_db = PostDb::from_dir(&in_dir.join("posts")).context("building post database")?;

	//write out the pages
	write_landing(&ramhorns, &post_db, &out_dir)?;
	write_discord(&ramhorns, &out_dir)?;
	
	Ok(())
}

/// Calls a function on every file in a directory, recursively.
/// Yoinked from the Rust documentation for std::fs::read_dir.
pub fn recursively_iterate_directory(dir: &Path, callback: &mut dyn FnMut(&fs::DirEntry) -> Result<()>) -> Result<()> {
	if dir.is_dir() {
		for entry in fs::read_dir(dir)? {
			let entry = entry?;
			if entry.path().is_dir() {
				recursively_iterate_directory(&entry.path(), callback)?;
			} else {
				callback(&entry)?;
			}
		}
	}

	Ok(())
}

/// Copy files from the in_dir to the out_dir.
fn copy_static(in_dir: &Path, out_dir: &Path) -> Result<()> {
	eprintln!("Copying static resources");
	
	if !in_dir.exists() {
		eprintln!("Not copying static files - {} does not exist.", in_dir.display());
		return Ok(());
	}

	let prefix_len = in_dir.components().count();

	recursively_iterate_directory(in_dir, &mut |entry| {
		//Chop off the .../whatever/in/static/ component of the path
		let dest_suffix = &entry.path().components().skip(prefix_len).collect::<PathBuf>();

		//Glue it onto the end of the .../whatever/out/static/ path
		let dest = &out_dir.join(dest_suffix);
		
		//Create the output directory. (Do it here, instead of before the loop, so subfolders exist.)
		let mut dest_dir = dest.clone();
		dest_dir.pop();
		
		fs::create_dir_all(&dest_dir).with_context(|| format!("creating static output directory at {}", dest_dir.display()))?;

		//Perform the copy operation
		let s = format!("Copying {} to {}", entry.path().display(), dest.display());
		eprintln!("{}", s);
		fs::copy(entry.path(), dest).context(s)?;

		Ok(())
	})
	.with_context(|| format!("iterating static resource input directory at {}", in_dir.display()))?;

	Ok(())
}

//Quick trait to make grabbing Ramhorns templates a tiny bit nicer
trait Yeet {
	fn render_and_write<C: Content>(&self, name: &'static str, context: &C, out_file: &Path) -> Result<()>;
}

impl Yeet for Ramhorns {
	fn render_and_write<C: Content>(&self, name: &'static str, context: &C, out_file: &Path) -> Result<()> {
        let template = self.get(name).context(format!("Missing template {}", name))?;
		let rendered = template.render(context);
		fs::write(out_file, rendered)?;
		Ok(())
    }
}

fn write_landing(templates: &Ramhorns, post_db: &PostDb, out_dir: &Path) -> Result<()> {
	eprintln!("Writing landing page");
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		posts: &'a Vec<&'a Post>
	}
	
	let ctx = TemplatingContext {
		posts: &post_db.all_posts.iter().filter(|post| !post.meta.draft).take(5).collect()
	};
	
	templates.render_and_write("index.template.html", &ctx, &out_dir.join("index.html"))?;
	Ok(())
}

fn write_discord(templates: &Ramhorns, out_dir: &Path) -> Result<()> {
	eprintln!("Writing discord page");
	templates.render_and_write("discord.template.html", &(), &out_dir.join("discord.html"))
}
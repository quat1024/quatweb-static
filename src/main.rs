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
use crate::post_db::Tag;

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
	copy_static(&in_dir.join("static"), &out_dir).context("copying static resources")?;

	//Build template engine
	eprintln!("Building template engine");
	let ramhorns = Ramhorns::from_folder(in_dir.join("templates")).context("initializing Ramhorns")?;
	
	//Build post database
	let post_db = PostDb::from_dir(&in_dir.join("posts")).context("building post database")?;

	//write out the pages
	write_landing(&ramhorns, &post_db, &out_dir)?;
	write_discord(&ramhorns, &out_dir)?;
	
	let posts_dir = out_dir.join("posts");
	fs::create_dir_all(&posts_dir).context("creating posts dir in output")?;
	write_post_index(&ramhorns, &post_db, &posts_dir)?;
	write_posts(&ramhorns, &post_db, &posts_dir)?;
	
	let tags_dir = out_dir.join("tags");
	fs::create_dir_all(&tags_dir).context("creating tags dir in output")?;
	write_tag_index(&ramhorns, &post_db, &tags_dir)?;
	write_tags(&ramhorns, &post_db, &tags_dir)?;
	
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

fn write_landing(templates: &Ramhorns, post_db: &PostDb, out_dir: &Path) -> Result<()> {
	eprintln!("Writing landing page");
	let template = templates.get("index.template.html").context("Missing template")?;
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		posts: &'a Vec<&'a Post>
	}
	
	let ctx = TemplatingContext {
		posts: &post_db.all_posts.iter().filter(|post| !post.meta.draft).take(5).collect()
	};
	
	let rendered = template.render(&ctx);
	fs::write(out_dir.join("index.html"), rendered)?;
	Ok(())
}

fn write_discord(templates: &Ramhorns, out_dir: &Path) -> Result<()> {
	eprintln!("Writing discord page(s)");
	let template = templates.get("discord.template.html").context("Missing template")?;
	let rendered = template.render(&());
	
	//Write to (out)/discord/index.html instead of (out)/discord.html.
	//If a link ends in a trailing slash, like "https://highlysuspect.agency/discord/",
	//Github Pages's router won't direct it to (out)/discord.html. Only works with a subfolder/index.html pair.
	let folder = out_dir.join("discord");
	fs::create_dir_all(&folder)?;
	fs::write(folder.join("index.html"), &rendered)?;
	
	//But also write it to the old location. Can't hurt.
	fs::write(out_dir.join("discord.html"), &rendered)?;
	Ok(())
}

fn write_post_index(templates: &Ramhorns, post_db: &PostDb, posts_dir: &Path) -> Result<()> {
	eprintln!("Writing post index page");
	let template = templates.get("post_index.template.html").context("Missing post_index.template.html")?;
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		posts: &'a Vec<Post>,
		count: usize,
		many: bool
	}
	
	let rendered = template.render(&TemplatingContext {
		posts: &post_db.all_posts,
		count: post_db.all_posts.len(),
		many: post_db.all_posts.len() > 1
	});
	
	fs::write(posts_dir.join("index.html"), rendered)?;
	
	Ok(())
}

fn write_posts(templates: &Ramhorns, post_db: &PostDb, posts_dir: &Path) -> Result<()> {
	eprintln!("Writing post pages");
	
	let template = templates.get("post.template.html").context("Missing post.template.html")?;
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		post: &'a Post,
		newer_post: &'a Option<&'a Post>,
		older_post: &'a Option<&'a Post>,
	}
	
	for post in post_db.all_posts.iter() {
		eprintln!("Writing post {}", &post.meta.title);
		
		let rendered = template.render(&TemplatingContext {
			post,
			newer_post: &post.meta.newer_post.and_then(|id| post_db.get_by_id(id)),
			older_post: &post.meta.older_post.and_then(|id| post_db.get_by_id(id)),
		});
		
		//rrrrarf
		let mut out_path = posts_dir.join(&post.meta.slug);
		out_path.set_extension("html");
		
		fs::write(out_path, rendered)?;
	}
	
	Ok(())
}

fn write_tag_index(templates: &Ramhorns, post_db: &PostDb, tags_dir: &Path) -> Result<()> {
	eprintln!("Writing tag index page");
	
	let template = templates.get("tag_index.template.html").context("Missing tag_index.template.html")?;
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		tags: &'a Vec<&'a Tag>,
		count: usize,
		many: bool,
	}
	
	let tags = &post_db.tags().collect();
	let rendered = template.render(&TemplatingContext {
		tags,
		count: tags.len(),
		many: tags.len() > 1
	});
	
	fs::write(tags_dir.join("index.html"), rendered)?;
	Ok(())
}

fn write_tags(templates: &Ramhorns, post_db: &PostDb, tags_dir: &Path) -> Result<()> {
	eprintln!("Writing tag pages");
	
	let template = templates.get("tag.template.html").context("Missing tag.template.html")?;
	
	#[derive(Content)]
	struct TemplatingContext<'a> {
		posts: &'a Vec<&'a Post>,
		count: usize,
		many: bool,
		tag: &'a str
	}
	
	for tag in post_db.tags() {
		let posts = &post_db.get_by_tag(tag);
		
		let rendered = template.render(&TemplatingContext {
			posts,
			count: posts.len(),
			many: posts.len() > 1,
			tag: tag.as_ref()
		});
		
		//Tags may contain periods, so set_extension stuff will break.
		let out_path = tags_dir.join([tag.as_ref(), ".html"].concat());
		
		fs::write(out_path, rendered)?;
	}
	
	Ok(())
}
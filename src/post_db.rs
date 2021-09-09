use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Lines;
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use ramhorns::Content;

use crate::date::MyDate;
use crate::markdown;
use crate::recursively_iterate_directory;

#[derive(Content, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Default, Debug)]
pub struct Tag(pub String);

impl AsRef<String> for Tag {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

#[derive(Content)]
pub struct Post {
	#[ramhorns(flatten)]
	pub meta: PostMeta,
	pub rendered_html: String,
}

#[derive(Content)]
pub struct PostMeta {
	#[ramhorns(skip)]
	pub input_path: PathBuf,
	/// The URL fragment. Determines where the file will live in the generated output.
	pub slug: String,
	pub author: String,
	pub title: String,
	pub blurb: Option<String>,
	pub created_date: MyDate,
	pub modified_date: Option<MyDate>,
	pub draft: bool,
	pub tags: Vec<Tag>,
	/// The index of the post posted after this one, in the PostDb.
	pub newer_post: Option<usize>,
	/// The index of the post posted before this one, in the PostDb.
	pub older_post: Option<usize>,
}

#[derive(Content)]
pub struct PostDb {
	pub all_posts: Vec<Post>,

	//indices into the map. NB: deriving Content for this whole struct is a bit overkill.
	#[ramhorns(skip)]
	pub posts_by_slug: HashMap<String, usize>,
	#[ramhorns(skip)]
	pub posts_by_tag: HashMap<Tag, Vec<usize>>,
}

impl Post {
	pub fn from_path(path: &Path) -> Result<Post> {
		let file = fs::File::open(path)?;
		let mut lines_reader = BufReader::new(file).lines();

		let meta = PostMeta::read_meta(path, &mut lines_reader)?;
		let rendered_html = Post::read_body(&mut lines_reader)?;

		Ok(Post { meta, rendered_html })
	}

	pub fn read_body<B: BufRead>(lines_reader: &mut Lines<B>) -> Result<String> {
		//Copy the entire rest of the file, because lmao Rust.
		//Once I've turned my BufReader into a lines reader, I can't recover the original reader,
		//and Lines doesn't implement Read or anything, it literally just has Debug and this line iterator available.
		//(Tokio's reader can be converted back, via into_inner. Is this something the stdlib is missing for a reason?)
		let rest = lines_reader.collect::<Result<Vec<String>, _>>()?.join("\n");
		Ok(markdown::render_to_html(&rest))
	}
}

impl PostMeta {
	pub fn read_meta<B: BufRead>(input_path: &Path, line_reader: &mut Lines<B>) -> Result<PostMeta> {
		let mut kv: HashMap<String, String> = HashMap::new();

		//Parse out each line and toss it into a hashmap.
		for line in line_reader {
			let line = line?;
			
			//Stop if I see three dashes. This consumes the line, preparing the reader for the post body
			if line == "---" {
				break;
			}

			if line.trim().is_empty() || matches!(line.chars().next(), Some('#')) {
				continue;
			}

			let equal_sign = line.find('=').context("needs equal sign")?;
			kv.insert(line[..equal_sign].to_string(), line[equal_sign + 1..].trim().to_string());
		}

		//Extract keys from the hashmap and throw them in the struct
		//Halfway through writing this im like.... Okay maybe something like Serde would be a good idea
		//Oh well

		//Annoying situation (empty is fine; if it's there it has to parse though)
		let modified_date: Option<Result<MyDate, _>> = kv.remove("modified_date").map(|x| x.parse());
		if let Some(Err(e)) = modified_date {
			bail!("last-modified date parsing error: {}", e);
		}
		let modified_date = modified_date.map(|x| x.unwrap());

		Ok(PostMeta {
			input_path: input_path.to_owned(),
			slug: kv.remove("slug").context("no slug")?,
			author: kv.remove("author").context("no author")?,
			title: kv.remove("title").context("no title")?,
			blurb: kv.remove("description"),
			created_date: kv.remove("created_date").context("no created date")?.parse().context("can't parse created date")?,
			modified_date,
			draft: kv.remove("draft").and_then(|x| x.parse().ok()).unwrap_or(false),
			tags: kv.remove("tags").unwrap_or_else(|| "".into()).split(',').map(|x| Tag(x.trim().to_owned())).collect(),
			newer_post: None, //to be filled in later
			older_post: None, //to be filled in later
		})
	}
}

impl PostDb {
	pub fn from_dir(path: &Path) -> Result<PostDb> {
		eprintln!("Building post database");
		
		let mut all_posts = Vec::new();

		recursively_iterate_directory(path, &mut |entry| {
			let s = format!("parsing post at {}", entry.path().display());
			eprintln!("{}", s);
			all_posts.push(Post::from_path(&entry.path()).context(s)?);
			Ok(())
		})?;

		all_posts.sort_by(|a, b| b.meta.created_date.cmp(&a.meta.created_date));

		//Populate next and previous post fields
		let mut i = all_posts.iter_mut().enumerate().peekable();
		while let Some((a_idx, a)) = i.next() {
			if let Some((b_idx, b)) = i.peek_mut() {
				a.meta.older_post = Some(*b_idx);
				b.meta.newer_post = Some(a_idx);
			}
		}

		let mut posts_by_slug = HashMap::new();
		let mut posts_by_tag: HashMap<_, Vec<_>> = HashMap::new();

		for (idx, post) in all_posts.iter().enumerate() {
			if posts_by_slug.insert(post.meta.slug.clone(), idx).is_some() {
				bail!("Duplicate post slug: {}", post.meta.slug);
			}

			for tag in post.meta.tags.iter() {
				(*posts_by_tag.entry(tag.clone()).or_default()).push(idx);
			}
		}

		Ok(PostDb { all_posts, posts_by_slug, posts_by_tag })
	}
	
	pub fn get_by_id(&self, id: usize) -> Option<&Post> {
		self.all_posts.get(id)
	}
	
	pub fn tags(&self) -> impl Iterator<Item = &Tag> {
		self.posts_by_tag.keys()
	}
	
	//Super lameo cloning operation
	pub fn get_by_tag(&self, tag: &Tag) -> Vec<&Post> {
		if let Some(post_ids) = self.posts_by_tag.get(tag) {
			post_ids.iter().map(|id| &self.all_posts[*id]).collect()
		} else {
			Vec::new()
		}
	}
}

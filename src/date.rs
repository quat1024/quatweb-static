use std::ops::Deref;
use std::str::FromStr;

use chrono::NaiveDate;
use ramhorns::Content;
use serde_with::DeserializeFromStr;

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, DeserializeFromStr)]
pub struct MyDate {
	inner: NaiveDate,
}

static EPIC_DATE_FORMAT: &str = "%b %d, %Y";

impl FromStr for MyDate {
	type Err = chrono::ParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(NaiveDate::parse_from_str(s, EPIC_DATE_FORMAT)?.into())
	}
}

impl From<NaiveDate> for MyDate {
	fn from(n: NaiveDate) -> Self {
		MyDate { inner: n }
	}
}

impl Deref for MyDate {
	type Target = NaiveDate;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl ToString for MyDate {
	fn to_string(&self) -> String {
		self.format(EPIC_DATE_FORMAT).to_string()
	}
}

impl Content for MyDate {
	fn render_escaped<E: ramhorns::encoding::Encoder>(&self, encoder: &mut E) -> Result<(), E::Error> {
		encoder.write_unescaped(&self.to_string())
	}
}

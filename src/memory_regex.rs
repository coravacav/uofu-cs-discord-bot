use regex::{Error, Regex};

#[derive(Clone, Debug)]
pub struct MemoryRegex {
    regex: Regex,
    pattern: String,
}

impl PartialEq for MemoryRegex {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for MemoryRegex {}

impl MemoryRegex {
    pub fn new(pattern: String) -> Result<Self, Error> {
        Ok(Self {
            regex: Regex::new(&pattern)?,
            pattern,
        })
    }
}

impl std::ops::Deref for MemoryRegex {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.regex
    }
}

impl std::ops::DerefMut for MemoryRegex {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.regex
    }
}

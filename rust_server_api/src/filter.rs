use std::{
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
};

pub struct Filter {
    words: Vec<String>
}

impl Filter {
    pub fn new(filename: impl AsRef<Path>) -> Result<Filter, anyhow::Error> {
        let file = File::open(filename).expect("no such file");
        let buf = BufReader::new(file);
        let words = buf.lines()
            .map(|l| l.expect("Could not parse line"))
            .collect();
        Ok(Filter{words})
    }
    pub fn is_unsafe(&self, input: &str) -> bool {
        let input = format!(" {} ", input.to_lowercase().replace(&['(', ')', ',', '\"', '.', ';', ':', '\'', '!', '@', '#', '$', '%', '^', '&', '*', '-', '_', '+', '=', '`', '~', '\n', '\r', '\\', '/', '{', '}', '°', '’', '‘', '>', '<', '»', '¢', '?'][..], " "));
        // println!("{}", input);
        for word in &self.words {
            let check_str = format!(" {} ", word);
            // println!("Checking: '{}' with '{}'.", &input, &check_str);
            if input.contains(&check_str) {
                // println!("Foundw word!");
                return true;
            }
        }
        return false;
    }
}
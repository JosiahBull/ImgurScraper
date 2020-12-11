///This module is designed to check if forbidden words exist in a given string.

//Imports
use std::{
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
};

///This struct holds a vector of forbidden words, along with methods for scanning.
pub struct Filter {
    words: Vec<String>
}

impl Filter {
    ///Creates a new filter struct.
    pub fn new(filename: impl AsRef<Path>) -> Result<Filter, anyhow::Error> {
        //Open and read the given input file, loading the list of forbidden words.
        let file = File::open(filename).expect("no such file");
        let buf = BufReader::new(file);
        let words = buf.lines()
            .map(|l| l.expect("Could not parse line"))
            .collect();
        Ok(Filter{words})
    }
    ///Takes a pointer to a string, and returns a boolean which determines whether or not the input string contains any forbidden words.
    pub fn is_unsafe(&self, input: &str) -> bool {
        //Filter the input string, removing puncutation and other non-ascii chars.
        let input = format!(" {} ", input.to_lowercase().replace(&['(', ')', ',', '\"', '.', ';', ':', '\'', '!', '@', '#', '$', '%', '^', '&', '*', '-', '_', '+', '=', '`', '~', '\n', '\r', '\\', '/', '{', '}', '°', '’', '‘', '>', '<', '»', '¢', '?'][..], " "));
        //Loop over each word in the input string, checking it against each check word. If we find a match, return true.
        for word in &self.words {
            let check_str = format!(" {} ", word);
            if input.contains(&check_str) {
                return true;
            }
        }
        //By default return false.
        return false;
    }
}
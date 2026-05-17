/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use bstr::ByteSlice;

use grep_searcher::{
    Searcher,
    Sink,
    SinkMatch 
};

use grep_regex::{
    RegexMatcher
};

use ignore::WalkBuilder;


use nucleo::{
    Config,
    Matcher,
    Utf32Str
};

use rayon::prelude::*;

use std::{
    io
};

/*****************************************************
 * Types 
 *****************************************************/

#[derive(Debug, Clone)]
pub enum SearchItemType {
    FILE,
    TEXT,
    DIRECTORY
}

#[derive(Debug, Clone)]
pub struct SearchItem {
    display  : String,
    metadata : Option<String>,
    item_type: SearchItemType
}

struct LineCollector {
    path_str : String,
    lines    : Vec<SearchItem>
}

/*****************************************************
 * Implementations 
 *****************************************************/

impl SearchItem {

    pub fn new (search_text : &str, metadata : Option<&str>, item_type : SearchItemType) -> Self
    {
        Self {
            display  : search_text.to_string(),
            metadata : metadata.map(|text| text.to_string()),
            item_type: item_type
        }
    }

    pub fn display(&self) -> &str {
        &self.display
    }
}

impl Sink for LineCollector {
   
    type Error = io::Error;
    
    fn matched (&mut self, _searcher : &Searcher, mat : &SinkMatch<'_>) -> Result<bool, io::Error> {
        
        let content = String::from_utf8_lossy(mat.bytes().trim_end());
        let metadata = format!("{}:{}", self.path_str, mat.line_number().unwrap_or_default());

        self.lines.push(SearchItem::new(&content, Some(&metadata), SearchItemType::TEXT));

        Ok(true)
    }
}

/*****************************************************
 * Function Definitions 
 *****************************************************/

pub fn search (query : &str) -> Vec<SearchItem> {
    
    let mut items = Vec::new();
    let mut searcher = Searcher::new();

    for entry in WalkBuilder::new("./").build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        let path_str = path.to_string_lossy().into_owned();

        if entry.file_type().map_or(false, |ft| ft.is_dir()) {

            items.push(
                SearchItem::new(
                &path_str, 
                None, 
                SearchItemType::DIRECTORY
                )
            );
             
        } else if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        
            items.push(
                SearchItem::new(
                    &path_str, 
                    None, 
                    SearchItemType::FILE
                )
            );

            let mut collector = LineCollector {
                path_str: path_str.clone(),
                lines: Vec::new(),
            };

            let matcher = RegexMatcher::new(".*").unwrap();

            if searcher.search_path(matcher, path, &mut collector).is_ok() {
                items.extend(collector.lines);
            }
        }
    }

    let query = nucleo::pattern::Pattern::parse(
        query,
        nucleo::pattern::CaseMatching::Ignore,
        nucleo::pattern::Normalization::Smart,
    );

    let mut matches: Vec<(u32, SearchItem)> = items
        .into_par_iter()
        .filter_map(|item| {
            let mut local_matcher = Matcher::new(Config::DEFAULT);
            let mut buffer = Vec::new();
            let utf32_display = Utf32Str::new(&item.display, &mut buffer);
            
            query.score(utf32_display, &mut local_matcher).map(|score| (score, item))
        })
        .collect();

    matches.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    matches.iter().map(|(_score, item)| item.clone()).collect()

}

/*****************************************************
 * Tests
 *****************************************************/

#[cfg(test)]
mod tests {

    use super::*;
   
    #[test]
    fn test_search() { 
        let matches = search("main");

        // 7. Output findings
        for  item in matches.iter().take(10) {
            println!("{:?}", item);
        }
    }

}

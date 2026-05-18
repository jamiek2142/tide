/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use bstr::ByteSlice;

use grep_searcher::{
    BinaryDetection, SearcherBuilder, Searcher, Sink, SinkMatch 
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
    io, path::Path, sync::mpsc::{
            self, 
            Receiver
        }, thread::{self, JoinHandle}
};

/*****************************************************
 * Types 
 *****************************************************/

#[derive(Default, Debug, Clone, PartialEq)]
pub enum SearchItemType {
    FILE,
    #[default] 
    TEXT,
    DIRECTORY
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct SearchItem {
    display  : String,
    metadata : Option<String>,
    item_type: SearchItemType
}

struct LineCollector {
    path_str : String,
    lines    : Vec<SearchItem>
}


pub struct SearchHandle {
    pub rx : Receiver<(u32, SearchItem)>,
    pub t1 : JoinHandle<()>,
    pub t2 : JoinHandle<()>
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

    pub fn item_type(&self) -> SearchItemType {
        self.item_type.clone()
    }

    pub fn metadata (&self) -> Option<&str> {
        self.metadata.as_deref()
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

pub fn search (cwd : &Path, query : &str) -> SearchHandle {
         
    let (build_tx, build_rx) = crossbeam_channel::bounded(2048);
    let cwd = cwd.to_path_buf();
    
    let t1 = thread::spawn(move || {
        let walker = WalkBuilder::new(&cwd).build_parallel();

        walker.run(move || {

        let mut searcher = SearcherBuilder::new().binary_detection(BinaryDetection::quit(b'\x00')).build();    
        let thread_local_tx : crossbeam_channel::Sender<SearchItem> = build_tx.clone(); 
    
        Box::new(move |entry | { 
        
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return ignore::WalkState::Continue
            };

            let path = entry.path();
            let path_str = path.to_string_lossy().into_owned();

            if entry.file_type().map_or(false, |ft| ft.is_dir()) {

               let _ = thread_local_tx.send(
                    SearchItem::new(
                    &path_str, 
                    None, 
                    SearchItemType::DIRECTORY
                    )
                );
             
            } else if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        
                let _ = thread_local_tx.send(
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
                   
                    for line in collector.lines {
                        let _ = thread_local_tx.send(line);
                    }
                }
            }

            ignore::WalkState::Continue
        })
    });
    });

    let (tx, rx) = mpsc::channel();

    let query = query.to_string();
    
    let t2 = thread::spawn(move || {
        let query = nucleo::pattern::Pattern::parse(
            &query,
            nucleo::pattern::CaseMatching::Ignore,
            nucleo::pattern::Normalization::Smart,
        );

        build_rx.into_iter().par_bridge()
            .for_each_with((Matcher::new(Config::DEFAULT), Vec::new()),|(matcher, buffer),item| {
                
                let utf32_display = Utf32Str::new(&item.display, buffer);
            
                if let Some(score) = query.score(utf32_display, matcher) {
                    let _ = tx.send((score, item.clone())); 
                };
        });    
    });

    SearchHandle { rx, t1, t2 } 
}


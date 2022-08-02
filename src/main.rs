mod dirutils;

use std::{env, fs, path::PathBuf, collections::hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use timely::dataflow::{
    operators::{Exchange, Filter, Input, Inspect, Map, Probe},
    InputHandle,
};
use walkdir::{DirEntry, WalkDir};


fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn main() {
    timely::execute_from_args(env::args(), |worker| {
        let index = worker.index();
        let peers = worker.peers();
        let mut input = InputHandle::new();

        worker.dataflow::<u64, _, _>(|scope| {
            scope
                .input_from(&mut input)
                // .filter(move |(i, _entry)| i % peers == index)
                // .map(|(_i, entry)| entry)
                .exchange(|entry: &PathBuf| {
                    let entry_string = entry.as_os_str().to_str().unwrap().to_string();
                    calculate_hash(&entry_string)
                })
                .map(dirutils::SearchResult::try_from)
                .filter(Result::is_ok)
                .map(Result::unwrap)
                .map(dirutils::SearchResult::with_icon)
                .inspect(|search_result| {
                    println!("{} - {:?}", search_result.name, search_result.icon_path)
                })
                .probe()
        });

        if index == 0 {
            let mut i = 0;
            for folder in dirutils::search_dirs().iter() {
                for entry in WalkDir::new(folder)
                    .into_iter()
                    .filter_entry(|e| !dirutils::is_hidden(e))
                    .filter_map(Result::ok)
                    .filter(dirutils::is_desktop_file)
                    .map(DirEntry::into_path)
                {
                    input.send(entry);
                    input.advance_to(i as u64 + 1);
                    worker.step();
                    i += 1;
                }
            }

            if let Some(paths) = env::var_os("PATH") {
                for path in env::split_paths(&paths) {
                    if let Ok(entries) = fs::read_dir(path) {
                        for entry in entries.filter_map(Result::ok).map(|e| e.path()) {
                            input.send(entry);
                            input.advance_to(i as u64 + 1);
                            worker.step();
                            i += 1;
                        }
                    }
                }
            }
        }
    })
    .unwrap();
}

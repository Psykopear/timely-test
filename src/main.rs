mod dirutils;

use std::hash::{Hash, Hasher};
use std::{collections::hash_map::DefaultHasher, env, fs, path::PathBuf};

use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::generic::source;
use timely::dataflow::operators::Operator;
use timely::dataflow::{
    operators::{aggregation::Aggregate, Exchange, Filter, Input, Inspect, Map, Probe},
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

        worker.dataflow::<u64, _, _>(|scope| {
            source(scope, "Dirutils", |cap, info| {
                let mut cap = Some(cap);
                move |output| {
                    if let Some(cap) = cap.as_mut() {
                        let time = cap.time().clone();

                        if index == 0 {
                            for folder in dirutils::search_dirs().iter() {
                                output.session(&cap).give_iterator(
                                    WalkDir::new(folder)
                                        .into_iter()
                                        .filter_entry(|e| !dirutils::is_hidden(e))
                                        .filter_map(Result::ok)
                                        .filter(dirutils::is_desktop_file)
                                        .map(DirEntry::into_path),
                                );
                            }

                            if let Some(paths) = env::var_os("PATH") {
                                for path in env::split_paths(&paths) {
                                    if let Ok(entries) = fs::read_dir(path) {
                                        output.session(&cap).give_iterator(
                                            entries.filter_map(Result::ok).map(|e| e.path()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    cap = None;
                }
            })
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
    })
    .unwrap();
}

mod dirutils;

use std::hash::{Hash, Hasher};
use std::thread;
use std::time::Duration;
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
        let mut user_input = InputHandle::<u64, String>::new();

        worker.dataflow::<u64, _, _>(|scope| {
            source(scope, "Dirutils", |cap, _info| {
                let mut cap = Some(cap);
                move |output| {
                    if let Some(cap) = cap.as_mut() {
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
            .binary(
                &user_input.to_stream(scope),
                Pipeline,
                Pipeline,
                "UserInput",
                |cap, _info| {
                    let mut cap = Some(cap);

                    move |results, user_input, output| {
                        let mut query_string = None;
                        while let Some((_time, data)) = user_input.next() {
                            for string in data.iter() {
                                println!("==> {}", string);
                                query_string = Some(string.clone());
                            }
                        }
                        if let Some(qs) = query_string {
                            // println!("qs: {}", qs);

                            if let Some(cap) = cap.take() {
                                while let Some((_time, data)) = results.next() {
                                    for sr in data.iter() {
                                        if sr.name.to_lowercase().contains(&qs) {
                                            output.session(&cap).give(sr.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            )
            .inspect(|search_result| {
                println!("{} - {:?}", search_result.name, search_result.icon_path)
            })
            .probe()
        });

        user_input.send("firefox".to_string());
        // user_input.advance_to(1u64);
        worker.step();
        thread::sleep(Duration::from_secs(1));
        user_input.send("nautilus".to_string());
        // user_input.advance_to(2u64);
        worker.step();
    })
    .unwrap();
}

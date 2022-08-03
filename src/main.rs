mod dirutils;

use std::hash::{Hash, Hasher};
use std::{collections::hash_map::DefaultHasher, env, fs, path::PathBuf};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use timely::dataflow::channels::pact::Pipeline;
use timely::dataflow::operators::generic::source;
use timely::dataflow::operators::Operator;
use timely::dataflow::{
    operators::{Exchange, Filter, Inspect, Map, Probe},
    InputHandle,
};

fn hash_pathbuf(t: &PathBuf) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn main() {
    timely::execute_from_args(env::args(), |worker| {
        let index = worker.index();
        let mut user_input = InputHandle::<u64, String>::new();
        let matcher = SkimMatcherV2::default();
        let mut cache: Vec<dirutils::SearchResult> = Vec::new();

        worker.dataflow::<u64, _, _>(|scope| {
            source(scope, "Dirutils", |cap, _info| {
                let mut cap = Some(cap);
                move |output| {
                    if index == 0 {
                        if let Some(cap) = cap.take() {
                            output.session(&cap).give_iterator(dirutils::paths_iter())
                        }
                    }
                }
            })
            .exchange(hash_pathbuf)
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
                    move |results, user_input, output| {
                        let mut query_string = None;
                        while let Some((_time, data)) = user_input.next() {
                            for string in data.iter() {
                                // println!("==> {}", string);
                                query_string = Some(string.clone());
                            }
                        }
                        while let Some((_time, data)) = results.next() {
                            for sr in data.iter() {
                                cache.push(sr.clone());
                            }
                        }

                        if let Some(qs) = query_string {
                            for sr in cache.iter() {
                                let mut search_name = String::from(&sr.name);
                                if let Some(path) = sr.desktop_entry_path.as_ref() {
                                    if let Some(file_name) = path.file_stem() {
                                        search_name.push(' ');
                                        search_name.push_str(file_name.to_str().unwrap_or(""));
                                    }
                                };
                                let result = matcher.fuzzy_indices(&search_name, &qs);

                                if let Some((score, indices)) = result {
                                    output.session(&cap).give(dirutils::SearchResult {
                                        // Always put desktop entry files first
                                        score: if sr.desktop_entry_path.is_some() {
                                            score + 1000
                                        } else {
                                            score
                                        },
                                        indices,
                                        ..sr.clone()
                                    })
                                }
                            }
                        }
                    }
                },
            )
            .inspect(|search_result| {
                println!(
                    "{} - {:?} <{}>",
                    search_result.name, search_result.desktop_entry_path, search_result.score
                )
            })
            .probe()
        });

        if index == 0 {
            user_input.send("".to_string());
            user_input.advance_to(1);
            worker.step();
            let mut i = 2;
            loop {
                println!("");
                println!("==> Write query: ");
                let mut user_in = String::new();
                std::io::stdin().read_line(&mut user_in).unwrap();
                user_input.send(user_in.trim().to_string());
                user_input.advance_to(i);
                worker.step();
                i += 1;
            }
        }
    })
    .unwrap();
}

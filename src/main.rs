mod dirutils;

use std::{env, io::stdin, thread};

use crossbeam::channel::{bounded, Receiver};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Exchange, Filter, Inspect, Map, Operator, Probe},
    InputHandle,
};

fn run_timely(rx: Receiver<String>) {
    timely::execute_from_args(env::args(), move |worker| {
        let index = worker.index();
        let mut user_input = InputHandle::<u64, String>::new();
        let matcher = SkimMatcherV2::default();
        let mut cache: Vec<dirutils::SearchResult> = Vec::new();

        let rx = rx.clone();

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
            .exchange(dirutils::hash_pathbuf)
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
                        // Add new elements to the cache
                        results.for_each(|_time, data| cache.extend_from_slice(&data));

                        // Get latest query string
                        let mut query_string = None;
                        user_input.for_each(|_time, data| {
                            query_string = data.last().cloned();
                        });

                        if let Some(qs) = query_string {
                            output
                                .session(&cap)
                                .give_iterator(cache.iter().cloned().filter_map(|sr| {
                                    matcher
                                        .fuzzy_match(&sr.search_name, &qs)
                                        .map(|score| sr.with_score(score))
                                }));
                        }
                    }
                },
            )
            .inspect(|search_result| {
                println!(
                    "===> <score: {}>\t{}",
                    search_result.score, search_result.name
                )
            })
            .probe()
        });

        user_input.send("".to_string());
        user_input.advance_to(1);
        worker.step();
        let mut i = 2;
        loop {
            if let Ok(msg) = rx.recv() {
                user_input.send(msg);
                user_input.advance_to(i);
                worker.step();
                i += 1;
            }
        }
    })
    .unwrap();
}

fn main() {
    let (tx, rx) = bounded::<String>(1);
    thread::spawn(|| run_timely(rx));
    loop {
        println!("Input query: ");
        let mut user_in = String::new();
        stdin().read_line(&mut user_in).unwrap();
        tx.send(user_in.trim().to_string()).unwrap();
    }
}

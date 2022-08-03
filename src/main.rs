mod dirutils;

use std::env;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Exchange, Filter, Inspect, Map, Operator, Probe},
    InputHandle,
};

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
                        let mut query_string = None;
                        // Get latest query string
                        user_input.for_each(|_time, data| {
                            query_string = data.last().cloned();
                        });
                        // Add new elements to the cache
                        results.for_each(|_time, data| cache.extend_from_slice(&data));

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
            .inspect(|search_result| println!("{} <{}>", search_result.name, search_result.score))
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

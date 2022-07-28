mod dirutils;

use std::{env, fs};

use dirutils::{is_desktop_file, is_hidden, search_dirs, SearchResult};

use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Filter, Inspect, Map, Operator},
};
use walkdir::WalkDir;

fn main() {
    timely::execute_from_args(env::args(), |worker| {
        let index = worker.index();
        let peers = worker.peers();

        worker.dataflow::<u64, _, _>(|scope| {
            // This is our source operator.
            // It goes through xdg data folders and emits each one of them
            source(scope, "data_dirs", |capability, info| {
                // Do I need to use the activator?
                // It works without it too, but I'm not sure what it does
                use timely::scheduling::Scheduler;
                let activator = scope.activator_for(&info.address[..]);

                // I thought I could just drop `capability` at the end of the closure, but that does
                // not work, so I'm capturing it in an Option like the book says, but I'd like to
                // understand what's happening here.
                let mut cap = Some(capability);

                move |output| {
                    if let Some(cap) = cap.as_mut() {
                        let time = cap.time().clone();
                        let mut i = 1_usize;
                        for folder in search_dirs() {
                            output.session(&cap).give((i, folder));
                            // output.session(&cap).give_vec(&mut search_dirs());
                            cap.downgrade(&(time + i as u64));
                            // Should I activate for every folder, or at the end of this loop, if at
                            // all?
                            activator.activate();
                            i += 1;
                        }

                        let key = "PATH";
                        if let Some(paths) = env::var_os(key) {
                            for path in env::split_paths(&paths) {
                                if let Ok(entries) = fs::read_dir(path) {
                                    output.session(&cap).give_iterator(
                                        &mut entries
                                            .filter_map(Result::ok)
                                            .map(|e| e.path())
                                            .enumerate(),
                                    );
                                    cap.downgrade(&(time + i as u64));
                                    activator.activate();
                                    i += 1;
                                }
                            }
                        }
                        // cap.downgrade(&(time + 1));
                    }
                    cap = None;
                }
            })
            .filter(move |(i, _folder)| i % peers == index)
            // Operator to extract files from the input directories
            .unary(Pipeline, "extract_files", |_capability, _info| {
                // Why do I need to capture the data inside a vec owned by this pipeline?
                // Why can't I drain it directly?
                let mut vector = Vec::new();
                let mut cap = Some(_capability);

                move |input, output| {
                    if let Some(cap) = cap.as_mut() {
                        let cap_time = cap.time().clone();
                        while let Some((time, data)) = input.next() {
                            data.swap(&mut vector);
                            let mut session = output.session(&time);
                            for (i, folder) in vector.drain(..) {
                                for entry in WalkDir::new(folder)
                                    .into_iter()
                                    .filter_entry(|e| !is_hidden(e))
                                    .filter_map(Result::ok)
                                    .filter(is_desktop_file)
                                {
                                    session.give(entry.into_path());
                                    cap.downgrade(&(cap_time + i as u64));
                                }
                            }
                        }
                    }
                    cap = None;
                }
            })
            .map(SearchResult::try_from)
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .map(SearchResult::with_icon)
            .inspect(|search_result| {
                println!("{} - {:?}", search_result.name, search_result.icon_path)
            });
        })
    })
    .unwrap();
}

mod dirutils;

use std::{env, fs};

use dirutils::{is_desktop_file, is_hidden, search_dirs, SearchResult};

use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Filter, Inspect, Map, Operator},
};
use walkdir::WalkDir;

fn main() {
    timely::example(|scope| {
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
                    let mut i = 1;
                    for folder in search_dirs() {
                        output.session(&cap).give(folder);
                        cap.downgrade(&(time + i));
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
                                    &mut entries.filter_map(Result::ok).map(|e| e.path()),
                                );
                                cap.downgrade(&(time + i));
                                activator.activate();
                                i += 1;
                            }
                        }
                    }
                }
                cap = None;
            }
        })
        // Operator to extract files from the input directories
        .unary(Pipeline, "extract_files", |_capability, _info| {
            // Why do I need to capture the data inside a vec owned by this pipeline?
            // Why can't I drain it directly?
            let mut vector = Vec::new();

            move |input, output| {
                while let Some((time, data)) = input.next() {
                    data.swap(&mut vector);
                    let mut session = output.session(&time);
                    for folder in vector.drain(..) {
                        for entry in WalkDir::new(folder)
                            .into_iter()
                            .filter_entry(|e| !is_hidden(e))
                            .filter_map(Result::ok)
                            .filter(is_desktop_file)
                        {
                            session.give(entry.into_path());
                        }
                    }
                }
            }
        })
        .map(SearchResult::try_from)
        .filter(Result::is_ok)
        .map(Result::unwrap)
        .map(SearchResult::with_icon)
        .inspect(|search_result| {
            println!("{} - {:?}", search_result.name, search_result.icon_path)
        });
    });
}

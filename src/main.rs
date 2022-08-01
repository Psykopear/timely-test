mod dirutils;

use std::{env, fs};

use dirutils::{is_desktop_file, is_hidden, search_dirs, SearchResult};

use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Filter, Input, Inspect, Map, Operator, Probe},
    InputHandle,
};
use walkdir::{WalkDir, DirEntry};

fn main() {
    timely::execute_from_args(env::args(), |worker| {
        let index = worker.index();
        let peers = worker.peers();
        let mut input = InputHandle::new();

        let probe = worker.dataflow::<u64, _, _>(|scope| {
            // This is our source operator.
            // It goes through xdg data folders and emits each one of them
            // source(scope, "data_dirs", |capability, _info| {
            //     move |output| {
            //         for folder in search_dirs() {
            //             output.session(&capability).give(folder);
            //         }
            //
            //         let key = "PATH";
            //         if let Some(paths) = env::var_os(key) {
            //             for path in env::split_paths(&paths) {
            //                 if let Ok(entries) = fs::read_dir(path) {
            //                     output.session(&capability).give_iterator(
            //                         &mut entries.filter_map(Result::ok).map(|e| e.path()),
            //                     );
            //                 }
            //             }
            //         }
            //     }
            // })
            scope
                .input_from(&mut input)
                // Operator to extract files from the input directories
                // .unary(Pipeline, "extract_files", |_capability, _info| {
                //     // Why do I need to capture the data inside a vec owned by this pipeline?
                //     // Why can't I drain it directly?
                //     let mut vector = Vec::new();
                //
                //     move |input, output| {
                //         while let Some((time, data)) = input.next() {
                //             data.swap(&mut vector);
                //             let mut session = output.session(&time);
                //             for folder in vector.drain(..) {
                //                 for entry in WalkDir::new(folder)
                //                     .into_iter()
                //                     .filter_entry(|e| !is_hidden(e))
                //                     .filter_map(Result::ok)
                //                     .filter(is_desktop_file)
                //                 {
                //                     session.give(entry.into_path());
                //                 }
                //             }
                //         }
                //     }
                // })
                .filter(move |(i, _entry)| i % peers == index)
                .map(|(_i, entry)| entry)
                .map(SearchResult::try_from)
                .filter(Result::is_ok)
                .map(Result::unwrap)
                .map(SearchResult::with_icon)
                .inspect(|search_result| {
                    println!("{} - {:?}", search_result.name, search_result.icon_path)
                })
                .probe()
        });

        let mut i = 0;
        for folder in search_dirs().iter() {
            for entry in WalkDir::new(folder)
                .into_iter()
                .filter_entry(|e| !is_hidden(e))
                .filter_map(Result::ok)
                .filter(is_desktop_file)
                .map(DirEntry::into_path)
                {
                    input.send((i, entry));
                    input.advance_to(i as u64 + 1);
                    worker.step();
                    i += 1;
                }
        }

        let key = "PATH";
        if let Some(paths) = env::var_os(key) {
            for path in env::split_paths(&paths) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.filter_map(Result::ok).map(|e| e.path()) {

                        input.send((i, entry));
                        input.advance_to(i as u64 + 1);
                        worker.step();
                        i += 1;
                    }
                }
            }
        }
    })
    .unwrap();
}

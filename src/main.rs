mod dirutils;

use dirutils::{is_desktop_file, is_hidden, search_dirs, searchresult_from_desktopentry};

use timely::dataflow::{
    channels::pact::Pipeline,
    operators::{generic::source, Filter, Inspect, Map, Operator},
};
use walkdir::WalkDir;

fn main() {
    timely::example(|scope| {
        source(scope, "data_dirs", |capability, info| {
            // Do I need to use the activator? It works without it too, but I'm not sure what it
            // does
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
                        // Should I activate for every folder, or at the end of it?
                        activator.activate();
                        i += 1;
                    }
                }
                cap = None;
            }
        })
        .unary(Pipeline, "extract_files", |_capability, _info| {
            let mut vector = Vec::new();
            move |input, output| {
                while let Some((time, data)) = input.next() {
                    data.swap(&mut vector);
                    let mut session = output.session(&time);
                    for folder in vector.drain(..) {
                        for entry in WalkDir::new(folder)
                            .into_iter()
                            .filter_entry(|e| !is_hidden(e))
                        {
                            if let Ok(entry) = entry {
                                if is_desktop_file(&entry) {
                                    session.give(entry.into_path());
                                }
                            }
                        }
                    }
                }
            }
        })
        .map(|entry| searchresult_from_desktopentry(&entry))
        .filter(|e| e.is_some())
        .map(|e| e.unwrap())
        .map(|mut e| {
            e.add_icon();
            e
        })
        .inspect(|search_result| {
            println!("{} - {:?}", search_result.name, search_result.icon_path)
        });
    });
}

use i3_window_killer::{external_command::*, formatter::*, parser::*};
use std::process;

fn main() {
    let raw_tree = get_tree().unwrap_or_else(|err| {
        println!("Error getting tree: [{}].", err);
        process::exit(1);
    });
    let tree = parse(raw_tree).unwrap_or_else(|err| {
        println!("Error parsing tree: [{}].", err);
        process::exit(1);
    });
    let node = find_focused(&tree).unwrap_or_else(|| {
        println!("Couldn't find focused node");
        process::exit(1);
    });
    let prompt = format(&node);
    if prompt_user(prompt) {
        kill();
    }
}

use i3_window_killer::{external_command::*, formatter::*, ipc_call::*, parser::*};
use i3ipc::I3Connection;

fn main() {
    let mut connection = I3Connection::connect().expect("failed to connect");
    let tree = get_tree(&mut connection).expect("failed to send command");
    let node = find_focused(&tree).expect("failed to find focused node");
    let prompt = format(&node);
    if prompt_user(prompt) {
        let outcomes = kill(&mut connection)
            .expect("failed to send command")
            .outcomes;
        for outcome in outcomes {
            if outcome.success {
                println!("success");
            } else {
                println!("failure");
                if let Some(e) = outcome.error.as_ref() {
                    println!("{}", e);
                }
            }
        }
    }
}

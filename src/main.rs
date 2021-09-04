use i3_window_killer::{external_command::*, formatter::*, ipc_call::*, parser::*};
use i3ipc::{reply::NodeType, I3Connection};

fn main() {
    let mut connection = I3Connection::connect().expect("failed to connect");
    let tree = get_tree(&mut connection).expect("failed to send command");
    let node = find_focused(&tree).expect("failed to find focused node");
    if node.nodetype == NodeType::Workspace && node.nodes.len() + node.floating_nodes.len() == 0 {
        return;
    }
    let (prompt, styles) = get_prompt_and_styles(&node);
    if prompt_user(prompt, styles) {
        let outcomes = kill(&mut connection)
            .expect("failed to execute command")
            .outcomes;
        for outcome in outcomes {
            if !outcome.success {
                println!("command did not succeed");
                if let Some(e) = outcome.error.as_ref() {
                    println!("{}", e);
                }
            }
        }
    }
}

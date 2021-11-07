use i3_ipc::{reply::NodeType, Connect, I3};
use i3_window_killer::{
    cli::get_options,
    external_command::{get_tree, kill, prompt_user},
    formatter::get_prompt_and_styles,
    utils::{fs::create_parent_dir, i3_tree::find_focused},
};

fn main() {
    let options = get_options();

    if let Some(ref path) = options.cache_file_path {
        if let Err(error) = create_parent_dir(path) {
            eprintln!("Couldn't create cache directory: {}", error)
        }
    }

    let mut con = I3::connect().expect("failed to connect");
    let tree = get_tree(&mut con).expect("failed to send command");
    let node = find_focused(&tree).expect("failed to find focused node");
    if node.node_type == NodeType::Workspace && node.nodes.len() + node.floating_nodes.len() == 0 {
        return;
    }
    let (prompt, styles) = get_prompt_and_styles(
        &node,
        &tree,
        options.rofi_theme_file,
        options.global_smart_gaps,
        options.global_outer_gap,
        options.cache_file_path,
    );
    if options.dump_styles {
        if let Some(ref styles) = styles {
            println!("{}", styles);
        }
    }
    if prompt_user(prompt, options.rofi_config, styles) {
        let outcomes = kill(&mut con).expect("failed to execute command");
        for outcome in outcomes {
            if !outcome.success {
                eprintln!("command did not succeed");
                if let Some(e) = outcome.error.as_ref() {
                    eprintln!("{}", e);
                }
            }
        }
    }
}

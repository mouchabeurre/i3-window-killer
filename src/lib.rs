pub mod ipc_call {
    use i3ipc::{
        reply::{Command, Node},
        I3Connection, MessageError,
    };

    pub fn get_tree(con: &mut I3Connection) -> Result<Node, MessageError> {
        con.get_tree()
    }

    pub fn kill(con: &mut I3Connection) -> Result<Command, MessageError> {
        con.run_command(&"kill".to_string())
    }
}

pub mod external_command {
    use std::env;
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn get_rofi_config_path() -> String {
        if let Some(os_string) = env::var_os("XDG_CONFIG_HOME") {
            if let Ok(path) = os_string.into_string() {
                format!("{}/rofi/config-kill.rasi", path)
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        }
    }

    pub fn prompt_user(prompt: String, styles: String) -> bool {
        const COMMAND: &str = "rofi";
        const YESNO: (&str, &str) = ("Yes", "No");
        let rofi_config_path = get_rofi_config_path();
        let args = [
            "-dmenu",
            "-auto-select",
            "-i",
            "-p",
            prompt.as_str(),
            "-config",
            rofi_config_path.as_str(),
            "-theme-str",
            styles.as_str(),
        ];
        let mut call = Command::new(COMMAND)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect(format!("failed to execute command: {} {}", COMMAND, args.join(" ")).as_str());
        {
            let stdin = call.stdin.as_mut().expect("failed to open stdin");
            stdin
                .write_all(format!("{}\n{}", YESNO.0, YESNO.1).as_bytes())
                .expect("failed to write to stdin");
        }
        let output = call.wait_with_output().expect("failed to read stdout");
        if let Ok(response) = String::from_utf8(output.stdout) {
            if response == format!("{}\n", YESNO.0) {
                return true;
            }
        }
        false
    }
}

pub mod parser {
    use i3ipc::reply::Node;

    pub fn find_focused(node: &Node) -> Option<&Node> {
        if node.focused {
            return Some(node);
        }
        if let Some(child) = node.nodes.iter().find(|&n| n.id == node.focus[0]) {
            if let Some(focused) = find_focused(child) {
                return Some(focused);
            }
        }
        if let Some(child) = node.floating_nodes.iter().find(|&n| n.id == node.focus[0]) {
            if let Some(focused) = find_focused(child) {
                return Some(focused);
            }
        }
        None
    }
}

pub mod formatter {
    use i3ipc::reply::{Node, WindowProperty};
    use ignore::WalkBuilder;
    use regex::Regex;

    #[derive(Debug)]
    struct NodeInfo {
        class: String,
        title: String,
        instance: Option<String>,
    }

    fn get_nodes_info(node: &Node) -> Vec<NodeInfo> {
        let mut nodes_info: Vec<NodeInfo> = Vec::new();
        if let Some(window_properties) = &node.window_properties {
            nodes_info.push(NodeInfo {
                class: window_properties
                    .get(&WindowProperty::Class)
                    .unwrap_or(&String::from("Unknown"))
                    .clone(),
                title: window_properties
                    .get(&WindowProperty::Title)
                    .unwrap_or(&String::from("Unknown"))
                    .clone(),
                instance: match window_properties.get(&WindowProperty::Instance) {
                    Some(instance) => Some(instance.clone()),
                    None => None,
                },
            });
        }
        for node in &node.nodes {
            nodes_info.append(get_nodes_info(&node).as_mut());
        }
        for node in &node.floating_nodes {
            nodes_info.append(get_nodes_info(&node).as_mut());
        }
        nodes_info
    }

    fn get_icon_by_class(class: &String) -> String {
        let re_desktop = Regex::new(format!(r"(?i:{}).*\.desktop$", class).as_str()).unwrap();
        let desktop_entries: Vec<String> = WalkBuilder::new("/usr/share/applications")
            .build()
            .filter_map(|entry| match entry {
                Ok(e) => {
                    if e.path().is_file() {
                        if let Some(path) = e.path().to_str() {
                            if re_desktop.is_match(path) {
                                Some(path.into())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .collect();
        println!("{:?}", desktop_entries);
        if desktop_entries.len() > 0 {
            let re_icon_desktop = Regex::new(r"^Icon=").unwrap();
            let text = std::fs::read_to_string(&desktop_entries[0]).unwrap();
            if let Some(icon_line) = text.lines().find(|l| re_icon_desktop.is_match(l)) {
                let icon_name = icon_line.split("=").collect::<Vec<&str>>()[1];
                return icon_name.trim().to_string();
            }
        }
        class.clone()
    }

    fn get_rofi_style(node: &Node, nodes_info: Vec<NodeInfo>) -> String {
        println!("{:?}", nodes_info);
        let mainbox_element = format!(
            "
        mainbox {{
            margin: {}px calc(100% - {}px) calc(100% - {}px) {}px;
        }}
        ",
            node.rect.1,
            node.rect.2 + node.rect.0,
            node.rect.3 + node.rect.1,
            node.rect.0
        );
        let nodes_container_ids: Vec<String> = (0..nodes_info.len())
            .map(|n| format!("node-{}", n))
            .collect();
        let nodes_elements: Vec<String> = nodes_info
            .iter()
            .zip(&nodes_container_ids)
            .map(|(n, id)| {
                let icon = get_icon_by_class(&n.class);
                format!(
                    "
            {} {{
                expand: false;
                spacing: 8;
                orientation: horizontal;
                children: [icon-{}, text-container-{}];
            }}
            text-container-{} {{
                expand: false;
                spacing: 8;
                orientation: horizontal;
                children: [textbox-class-{}, textbox-label-{}];
            }}
            icon-{} {{
                background-color: @normal-foreground;
                expand: false;
                padding: 2px;
                border-radius: 2px;
                filename: \"{}\";
                vertical-align: 0.5;
            }}
            textbox-class-{} {{
                expand: false;
                str: \"{}:\";
                font: \"Hack Bold 10\";
                vertical-align: 0.5;
                text-color: @normal-foreground;
            }}
            textbox-label-{} {{
                expand: false;
                str: \"{}\";
                font: \"Hack 10\";
                vertical-align: 0.5;
                text-color: @normal-foreground;
            }}
            ",
                    &id, &id, &id, &id, &id, &id, &id, &icon, &id, &n.class, &id, &n.title
                )
            })
            .collect();
        let nodes_parent_element = format!(
            "
        nodes {{
            children: [{}];
        }}
        ",
            &nodes_container_ids.join(",")
        );
        format!(
            "
        {}
        {}
        {}
        ",
            mainbox_element,
            nodes_parent_element,
            nodes_elements.join("\n")
        )
    }

    pub fn get_prompt_and_styles(node: &Node) -> (String, String) {
        let nodes_info = get_nodes_info(node);
        let prompt = format!("Close node{}", if nodes_info.len() > 1 { "s" } else { "" });
        let styles = get_rofi_style(node, nodes_info);
        (prompt, styles)
    }
}

pub mod ipc_call {
    use i3ipc::{
        reply::{Command, Node},
        I3Connection, MessageError,
    };

    pub fn get_tree(con: &mut I3Connection) -> Result<Node, MessageError> {
        con.get_tree()
    }

    pub fn kill(con: &mut I3Connection) -> Result<Command, MessageError> {
        let command_text = String::from("kill");
        con.run_command(&command_text)
    }
}

pub mod external_command {
    use std::io::Write;
    use std::process::{Command, Stdio};

    pub fn prompt_user(prompt: String) -> bool {
        const COMMAND: &str = "rofi";
        let args = [
            "-dmenu",
            "-auto-select",
            "-i",
            "-p",
            prompt.as_str(),
            "-theme-str",
            "mainbox { padding: 490px 730px; }",
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
                .write_all("Yes\nNo".as_bytes())
                .expect("failed to write to stdin");
        }
        let output = call.wait_with_output().expect("failed to read stdout");
        if let Ok(response) = String::from_utf8(output.stdout) {
            if response == "Yes\n" {
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
    use unicode_segmentation::UnicodeSegmentation;

    const MAX_LEN: usize = 50;
    const MIN_LEN: usize = 4;
    const SEPARATOR: &str = ",";
    const ELLIPSIS: &str = "...";
    const PARENS: [&str; 2] = ["(", ")"];
    const SPACE: &str = " ";
    pub const PREFIX: &str = "Close";

    #[derive(Debug)]
    struct WindowInfo {
        class: String,
        title: String,
    }

    fn get_window_info(node: &Node) -> Vec<WindowInfo> {
        let mut windows_info: Vec<WindowInfo> = Vec::new();
        if let Some(window_properties) = &node.window_properties {
            windows_info.push(WindowInfo {
                class: window_properties
                    .get(&WindowProperty::Class)
                    .unwrap_or(&String::from("Unknown"))
                    .clone(),
                title: window_properties
                    .get(&WindowProperty::Title)
                    .unwrap_or(&String::from("Unknown"))
                    .clone(),
            });
        }
        for node in &node.nodes {
            windows_info.append(get_window_info(&node).as_mut());
        }
        windows_info
    }

    fn truncate(s: &String, n: usize) -> String {
        s.graphemes(true).take(n).collect()
    }

    fn len(s: &String) -> usize {
        s.graphemes(true).count()
    }

    pub fn format(node: &Node) -> String {
        let _separator_len = len(&SEPARATOR.to_string());
        let ellipsis_len = len(&ELLIPSIS.to_string());
        let parens_len = len(&PARENS.concat().to_string());
        let space_len = len(&SPACE.to_string());
        let windows_info = get_window_info(node);
        let mut prompt: Vec<String> = vec![PREFIX.to_string(), SPACE.to_string()];

        if windows_info.len() == 1 {
            if let Some(info) = windows_info.iter().next() {
                let title = &info.title;
                let class = &info.class;
                let title_len = len(title);
                let class_len = len(class);
                let mut current_len = len(&prompt.concat());
                if current_len + class_len <= MAX_LEN {
                    prompt.push(class.clone());
                    current_len = len(&prompt.concat());
                    if current_len + space_len + parens_len + title_len <= MAX_LEN {
                        prompt.push(format!(
                            "{}{}{}{}",
                            SPACE,
                            PARENS[0],
                            title.clone(),
                            PARENS[1]
                        ));
                    } else {
                        let available_len =
                            MAX_LEN - (current_len + space_len + parens_len + ellipsis_len);
                        if available_len >= MIN_LEN {
                            prompt.push(format!(
                                "{}{}{}{}{}",
                                SPACE,
                                PARENS[0],
                                truncate(title, available_len),
                                ELLIPSIS,
                                PARENS[1]
                            ));
                        }
                    }
                } else {
                    let available_len = MAX_LEN - (current_len + ellipsis_len);
                    if available_len >= MIN_LEN {
                        prompt.push(format!("{}{}", truncate(class, available_len), ELLIPSIS));
                    } else {
                        if current_len + ellipsis_len <= MAX_LEN {
                            prompt.push(ELLIPSIS.to_string());
                        }
                    }
                }
            }
        } else {
            for (i, info) in windows_info.iter().enumerate() {
                let class = &info.class;
                let class_len = len(class);
                let current_len = len(&prompt.concat());
                let separator = if i == 0 { "" } else { SEPARATOR };
                let space = if i == 0 { "" } else { SPACE };
                let separator_len = len(&separator.to_string());
                let space_len = len(&space.to_string());
                if current_len + separator_len + space_len + class_len <= MAX_LEN {
                    if i == windows_info.len() - 1
                        || current_len
                            + separator_len
                            + space_len
                            + class_len
                            + separator_len
                            + space_len
                            + MIN_LEN
                            + ellipsis_len
                            <= MAX_LEN
                    {
                        prompt.push(format!("{}{}{}", separator, space, class));
                    } else {
                        if current_len + separator_len + space_len + class_len + ellipsis_len
                            <= MAX_LEN
                        {
                            prompt.push(format!(
                                "{}{}{}{}",
                                separator,
                                space,
                                class,
                                ELLIPSIS.to_string()
                            ));
                        } else {
                            let available_len =
                                MAX_LEN - (current_len + separator_len + space_len + ellipsis_len);
                            prompt.push(format!(
                                "{}{}{}{}",
                                separator,
                                space,
                                truncate(class, available_len),
                                ELLIPSIS
                            ));
                        }
                        break;
                    }
                } else {
                    if current_len + separator_len + space_len + MIN_LEN + ellipsis_len <= MAX_LEN {
                        let available_len =
                            MAX_LEN - (current_len + separator_len + space_len + ellipsis_len);
                        prompt.push(format!(
                            "{}{}{}{}",
                            separator,
                            space,
                            truncate(class, available_len),
                            ELLIPSIS
                        ));
                    } else {
                        if current_len + ellipsis_len <= MAX_LEN {
                            prompt.push(ELLIPSIS.to_string());
                        }
                    }
                    break;
                }
            }
        }
        prompt.concat()
    }
}

pub mod external_command {
    use std::io::Write;
    use std::process::{Command, Stdio};

    pub fn get_tree() -> Result<String, String> {
        const COMMAND: &str = "i3-msg";
        let args = ["-t", "get_tree"];
        let call = Command::new(COMMAND)
            .args(&args)
            .output()
            .expect(format!("Failed to execute command: {} {}", COMMAND, args.join(" ")).as_str());
        if call.status.success() {
            if let Ok(tree) = String::from_utf8(call.stdout) {
                return Ok(tree);
            }
        }
        Err(String::from("Couldn't get i3wm window tree"))
    }

    pub fn prompt_user(prompt: String) -> bool {
        const COMMAND: &str = "rofi";
        const PREFIX: &str = "Kill";
        let full_prompt = format!("{} {} ", PREFIX, prompt);
        let args = [
            "-dmenu",
            "-auto-select",
            "-i",
            "-p",
            full_prompt.as_str(),
            "-theme-str",
            "mainbox { padding: 490px 800px; }",
        ];
        let mut call = Command::new(COMMAND)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect(format!("Failed to execute command: {} {}", COMMAND, args.join(" ")).as_str());
        {
            let stdin = call.stdin.as_mut().expect("Failed to open stdin");
            stdin
                .write_all("Yes\nNo".as_bytes())
                .expect("Failed to write to stdin");
        }
        let output = call.wait_with_output().expect("Failed to read stdout");
        if let Ok(response) = String::from_utf8(output.stdout) {
            if response == "Yes\n" {
                return true;
            }
        }
        false
    }

    pub fn kill() {
        const COMMAND: &str = "i3-msg";
        let args = ["kill"];
        Command::new(COMMAND)
            .args(&args)
            .output()
            .expect(format!("Failed to execute command: {} {}", COMMAND, args.join(" ")).as_str());
    }
}

pub mod parser {
    use serde::{Deserialize, Serialize};
    use serde_json::Error as SerdeError;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Node {
        pub id: i64,
        pub focused: bool,
        pub name: Option<String>,
        pub window_properties: Option<WindowProperties>,
        pub nodes: Vec<Node>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct WindowProperties {
        pub class: String,
        pub instance: String,
        pub title: String,
    }

    pub fn parse(tree: String) -> Result<Node, SerdeError> {
        match serde_json::from_str::<Node>(tree.as_str()) {
            Ok(parsed) => Ok(parsed),
            Err(error) => Err(error),
        }
    }

    pub fn find_focused(node: &Node) -> Option<&Node> {
        // dfs
        for node in node.nodes.iter() {
            if node.focused {
                return Some(node);
            } else {
                if let Some(node) = find_focused(node) {
                    return Some(node);
                }
            }
        }
        None
    }
}

pub mod formatter {
    use crate::parser::Node;

    #[derive(Debug)]
    struct WindowInfo {
        class: String,
        title: String,
    }

    fn get_window_info(node: &Node) -> Vec<WindowInfo> {
        let mut windows_info: Vec<WindowInfo> = Vec::new();
        if let Some(window_properties) = &node.window_properties {
            windows_info.push(WindowInfo {
                class: window_properties.class.clone(),
                title: window_properties.title.clone(),
            });
        }
        for node in &node.nodes {
            windows_info.append(get_window_info(&node).as_mut());
        }
        windows_info
    }

    pub fn format(node: &Node) -> String {
        let windows_info = get_window_info(node);
        const MAX_LEN: usize = 30;
        const MIN_LEN: usize = 4;
        const SEPARATOR: &str = ", ";
        const ELLIPSIS: &str = "...";
        let mut prompt: Vec<String> = Vec::new();
        let mut windows_info_iter = windows_info.iter();
        let first_window_info = windows_info_iter.next();
        if let Some(info) = first_window_info {
            if info.class.len() <= MAX_LEN {
                prompt.push(info.class.clone());
            } else {
                let mut class = info.class.clone();
                class.truncate(MAX_LEN - ELLIPSIS.len());
                prompt.push(format!("{}{}", class.trim(), ELLIPSIS));
            }
        }
        if windows_info.len() > 1 {
            for info in windows_info_iter {
                let current_length = prompt.iter().fold(0, |sum, x| sum + x.len());
                if current_length + SEPARATOR.len() + info.class.len() <= MAX_LEN {
                    prompt.push(format!("{}{}", SEPARATOR, info.class));
                } else {
                    if current_length + SEPARATOR.len() + MIN_LEN + ELLIPSIS.len() <= MAX_LEN {
                        let mut class = info.class.clone();
                        class.truncate(MAX_LEN - ELLIPSIS.len());
                        prompt.push(format!("{}{}{}", SEPARATOR, class, ELLIPSIS));
                    } else if current_length + SEPARATOR.len() + ELLIPSIS.len() <= MAX_LEN {
                        prompt.push(format!("{}{}", SEPARATOR, ELLIPSIS));
                    }
                }
            }
        } else {
            const PARENS: [&str; 2] = ["(", ")"];
            const PADDING: &str = " ";
            let current_length = prompt.iter().fold(0, |sum, x| sum + x.len());
            if let Some(info) = first_window_info {
                if current_length + PADDING.len() + PARENS.join("").len() + info.title.len()
                    <= MAX_LEN
                {
                    prompt.push(format!(
                        "{}{}{}{}",
                        PADDING,
                        PARENS[0],
                        info.title.clone(),
                        PARENS[1]
                    ));
                } else if MAX_LEN
                    - current_length
                    - PADDING.len()
                    - PARENS.join("").len()
                    - ELLIPSIS.len()
                    >= MIN_LEN
                {
                    let mut title = info.title.clone();
                    title.truncate(
                        MAX_LEN
                            - current_length
                            - PADDING.len()
                            - PARENS.join("").len()
                            - ELLIPSIS.len(),
                    );
                    println!("{}", title);
                    prompt.push(format!(
                        "{}{}{}{}{}",
                        PADDING,
                        PARENS[0],
                        title.trim(),
                        ELLIPSIS,
                        PARENS[1]
                    ));
                }
            }
        }
        prompt.join("")
    }
}

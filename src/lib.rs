pub mod cli;
pub mod utils;

pub mod external_command {
    use i3_ipc::{reply, I3Stream};
    use std::io::{self, Write};
    use std::process::{Command, Stdio};

    pub fn get_tree(con: &mut I3Stream) -> io::Result<reply::Node> {
        con.get_tree()
    }

    pub fn kill(con: &mut I3Stream) -> io::Result<Vec<reply::Success>> {
        con.run_command(&"kill".to_string())
    }

    pub fn prompt_user(prompt: String, config: Option<String>, styles: Option<String>) -> bool {
        const COMMAND: &str = "rofi";
        const CHOICES: (&str, &str) = ("Yes", "No");
        let mut args = vec!["-dmenu", "-auto-select", "-i", "-p", prompt.as_str()];
        if let Some(ref config) = config {
            args.append(vec!["-config", config.as_str().clone()].as_mut());
        }
        if let Some(ref styles) = styles {
            args.append(vec!["-theme-str", styles.as_str().clone()].as_mut());
        }
        let mut call = Command::new(COMMAND)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect(format!("failed to execute command: {} {}", COMMAND, args.join(" ")).as_str());
        {
            let stdin = call.stdin.as_mut().expect("failed to open stdin");
            stdin
                .write_all(format!("{}\n{}", CHOICES.0, CHOICES.1).as_bytes())
                .expect("failed to write to stdin");
        }
        let output = call.wait_with_output().expect("failed to read stdout");
        if let Ok(response) = String::from_utf8(output.stdout) {
            if response == format!("{}\n", CHOICES.0) {
                return true;
            }
        }
        false
    }
}

pub mod formatter {
    use crate::{cli::SmartGapsOption, utils::i3_tree::get_child_iter};
    use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
    use i3_ipc::reply::{Node, NodeLayout, NodeType};
    use ignore::WalkBuilder;
    use regex::Regex;
    use serde::Serialize;
    use std::{
        collections::HashMap,
        fs::{self, OpenOptions},
        io::Write,
        path::PathBuf,
    };
    use tinytemplate::{format_unescaped, TinyTemplate};

    #[derive(Debug, Serialize)]
    struct TemplateContext {
        #[serde(rename(serialize = "container"))]
        container_rect: NodeRect,
        nodes: Vec<NodeInfo>,
    }

    #[derive(Debug, Serialize)]
    struct NodeRect {
        top: i32,
        right: i32,
        bottom: i32,
        left: i32,
    }

    #[derive(Debug, Serialize)]
    struct NodeInfo {
        class: String,
        title: String,
        icon: String,
    }

    fn get_nodes_info(node: &Node, cache_file_path: Option<PathBuf>) -> Vec<NodeInfo> {
        fn get_icon_by_class(
            class: &String,
            cache_path: &Option<PathBuf>,
            icon_map: &mut HashMap<String, String>,
        ) -> String {
            fn get_icon_by_class_from_cache(
                class: &String,
                cache_path: &Option<PathBuf>,
            ) -> Option<String> {
                if let Some(path) = cache_path {
                    match fs::read_to_string(path) {
                        Ok(text) => text.lines().find_map(|line| {
                            if line.starts_with(class) {
                                line.rsplit_once("=").map(|splits| splits.1.into())
                            } else {
                                None
                            }
                        }),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
            fn get_desktop_file_by_class(class: &String) -> Option<String> {
                let matcher = SkimMatcherV2::ignore_case(SkimMatcherV2::default());
                let re_desktop = Regex::new(r".*\.desktop$").unwrap();
                let mut matches_desktop: Vec<(String, i64)> =
                    WalkBuilder::new("/usr/share/applications")
                        .build()
                        .filter_map(|entry| match entry {
                            Ok(entry) => {
                                if entry.path().is_file() {
                                    if let Some(path) = entry.path().to_str() {
                                        if re_desktop.is_match(path) {
                                            if let Ok(text) = fs::read_to_string(path) {
                                                text.lines().find_map(|line| {
                                                    if line.starts_with("Name=") {
                                                        if let Some(name_value) = line
                                                            .split("=")
                                                            .collect::<Vec<&str>>()
                                                            .get(1)
                                                        {
                                                            if let Some(score) = matcher
                                                                .fuzzy_match(
                                                                    &name_value,
                                                                    class.as_str(),
                                                                )
                                                            {
                                                                Some((path.into(), score))
                                                            } else {
                                                                None
                                                            }
                                                        } else {
                                                            None
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                })
                                            } else {
                                                None
                                            }
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
                matches_desktop.sort_by(|a, b| b.1.cmp(&a.1));
                matches_desktop.get(0).map(|(file, _)| file.into())
            }
            if let Some(icon_name) = icon_map.get(class) {
                icon_name.into()
            } else {
                let icon_name = match get_icon_by_class_from_cache(class, cache_path) {
                    Some(icon_name) => icon_name,
                    None => {
                        let default_icon_name = class.clone();
                        let new_icon_name = if let Some(file) = get_desktop_file_by_class(class) {
                            if let Ok(text) = fs::read_to_string(file) {
                                if let Some(icon_line) =
                                    text.lines().find(|line| line.starts_with("Icon="))
                                {
                                    if let Some(icon_name) =
                                        icon_line.split("=").collect::<Vec<&str>>().get(1)
                                    {
                                        icon_name.trim().to_string()
                                    } else {
                                        default_icon_name
                                    }
                                } else {
                                    default_icon_name
                                }
                            } else {
                                default_icon_name
                            }
                        } else {
                            default_icon_name
                        };
                        if let Some(path) = cache_path {
                            match OpenOptions::new()
                                .create(true)
                                .write(true)
                                .append(true)
                                .open(path)
                            {
                                Ok(mut file) => {
                                    if let Err(_) = writeln!(file, "{}={}", class, new_icon_name) {
                                        eprintln!("couldn't write icon entry to cache");
                                    }
                                }
                                Err(_) => eprintln!("couldn't open cache file to write"),
                            }
                        }
                        new_icon_name
                    }
                };
                icon_map.insert(class.clone(), icon_name.clone());
                icon_name
            }
        }
        fn build_nodes_info(
            node: &Node,
            cache_path: &Option<PathBuf>,
            icon_map: &mut HashMap<String, String>,
        ) -> Vec<NodeInfo> {
            let mut nodes_info: Vec<NodeInfo> = Vec::new();
            if let Some(window_properties) = &node.window_properties {
                let class = window_properties
                    .class
                    .as_ref()
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let title = window_properties
                    .title
                    .as_ref()
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let icon = get_icon_by_class(&class, cache_path, icon_map);
                nodes_info.push(NodeInfo { class, title, icon });
            }
            get_child_iter(node).for_each(|node| {
                nodes_info.append(build_nodes_info(node, cache_path, icon_map).as_mut())
            });
            nodes_info
        }
        let mut icon_map: HashMap<String, String> = HashMap::new();
        build_nodes_info(node, &cache_file_path, &mut icon_map)
    }

    fn find_inherited_rect(
        target: &Node,
        node: &Node,
        global_smart_gaps: SmartGapsOption,
        global_outer_gap: Option<i32>,
    ) -> NodeRect {
        fn get_node_with_childs(node: &Node) -> Option<&Node> {
            if node.nodes.len() > 1 {
                Some(node)
            } else {
                node.nodes
                    .iter()
                    .find(|n| get_node_with_childs(n).is_some())
            }
        }
        fn can_workspace_of_node_have_gaps(
            target: &Node,
            node_chain: &Vec<&Node>,
            smart_gaps: SmartGapsOption,
        ) -> Option<bool> {
            if node_chain
                .iter()
                .zip(node_chain.iter().skip(1))
                .any(|(n1, n2)| n1.floating_nodes.iter().any(|n| n.id == n2.id))
            {
                // target is in a floating tree
                Some(false)
            } else {
                match node_chain
                    .iter()
                    .enumerate()
                    .find(|(_, n)| n.node_type == NodeType::Workspace)
                {
                    Some((workspace_index, workspace)) => {
                        if workspace.id == target.id {
                            match get_node_with_childs(workspace) {
                                Some(first_container) => match smart_gaps {
                                    SmartGapsOption::Off => Some(true),
                                    SmartGapsOption::On | SmartGapsOption::InverseOuter => {
                                        if first_container.layout == NodeLayout::Stacked
                                            || first_container.layout == NodeLayout::Tabbed
                                        {
                                            Some(false)
                                        } else {
                                            Some(true)
                                        }
                                    }
                                },
                                None => match smart_gaps {
                                    SmartGapsOption::Off => Some(true),
                                    SmartGapsOption::On => Some(false),
                                    SmartGapsOption::InverseOuter => Some(true),
                                },
                            }
                        } else {
                            match node_chain
                                .iter()
                                .skip(workspace_index)
                                .find(|n| n.nodes.len() > 1)
                            {
                                Some(first_container) => match smart_gaps {
                                    SmartGapsOption::Off => Some(true),
                                    SmartGapsOption::On | SmartGapsOption::InverseOuter => {
                                        if first_container.layout == NodeLayout::Stacked
                                            || first_container.layout == NodeLayout::Tabbed
                                        {
                                            Some(false)
                                        } else {
                                            Some(true)
                                        }
                                    }
                                },
                                None => match smart_gaps {
                                    SmartGapsOption::Off => Some(true),
                                    SmartGapsOption::On => Some(false),
                                    SmartGapsOption::InverseOuter => Some(true),
                                },
                            }
                        }
                    }
                    None => None,
                }
            }
        }
        fn get_node_rect(node: &Node, with_gaps: bool, global_outer_gap: Option<i32>) -> NodeRect {
            let mut x = node.rect.x as i32;
            let mut y = node.rect.y as i32;
            let mut width = node.rect.width as i32;
            let mut height = node.rect.height as i32;
            if with_gaps {
                if let Some(gaps) = &node.gaps {
                    x += gaps.left as i32;
                    y += gaps.top as i32;
                    width -= (gaps.left + gaps.right) as i32;
                    height -= (gaps.top + gaps.bottom) as i32;
                    if let Some(outer_gap) = global_outer_gap {
                        x += outer_gap;
                        y += outer_gap;
                        width -= outer_gap + outer_gap;
                        height -= outer_gap + outer_gap;
                    }
                }
            }
            NodeRect {
                top: y,
                right: (width + x),
                bottom: (height + y),
                left: x,
            }
        }
        fn get_node_chain<'a>(target: &Node, node: &'a Node) -> Option<Vec<&'a Node>> {
            if node.id == target.id {
                return Some(vec![node]);
            }
            get_child_iter(node).find_map(|n| match get_node_chain(target, n) {
                Some(chain) => Some(
                    vec![node]
                        .iter()
                        .chain(chain.iter())
                        .map(|n| *n)
                        .collect::<Vec<&'a Node>>(),
                ),
                None => None,
            })
        }
        let node_rect_default = get_node_rect(target, false, global_outer_gap);
        match get_node_chain(target, node) {
            Some(chain) => {
                let with_gaps =
                    match can_workspace_of_node_have_gaps(target, &chain, global_smart_gaps) {
                        Some(can_have_gaps) => can_have_gaps,
                        None => false,
                    };
                chain
                    .iter()
                    .rev()
                    .map(|n| get_node_rect(n, with_gaps, global_outer_gap))
                    .reduce(|a, b| NodeRect {
                        top: a.top.max(b.top),
                        right: a.right.min(b.right),
                        bottom: a.bottom.min(b.bottom),
                        left: a.left.max(b.left),
                    })
                    .unwrap_or(node_rect_default)
            }
            None => node_rect_default,
        }
    }

    fn get_rofi_styles(context: TemplateContext, template: String) -> String {
        const TEMPLATE_NAME: &str = "main";
        let mut tt = TinyTemplate::new();
        tt.set_default_formatter(&format_unescaped);
        if let Err(e) = tt.add_template(TEMPLATE_NAME, &template) {
            panic!("couldn't register template: {}", e);
        }
        match tt.render(TEMPLATE_NAME, &context) {
            Ok(rendered) => rendered,
            Err(e) => {
                panic!("couldn't render template: {}", e);
            }
        }
    }

    pub fn get_prompt_and_styles(
        node: &Node,
        tree: &Node,
        template: Option<PathBuf>,
        global_smart_gaps: SmartGapsOption,
        global_outer_gap: Option<i32>,
        cache_file_path: Option<PathBuf>,
    ) -> (String, Option<String>) {
        let nodes_info = get_nodes_info(node, cache_file_path);
        let prompt = format!("Close node{}", if nodes_info.len() > 1 { "s" } else { "" });
        let container_rect = find_inherited_rect(node, tree, global_smart_gaps, global_outer_gap);
        let context = TemplateContext {
            nodes: nodes_info,
            container_rect,
        };
        let styles = match template {
            Some(path) => {
                let contents = std::fs::read_to_string(path).expect("couldn't read template file");
                Some(get_rofi_styles(context, contents))
            }
            None => None,
        };
        (prompt, styles)
    }
}

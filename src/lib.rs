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
    use std::io::Write;
    use std::process::{Command, Stdio};

    pub fn prompt_user(prompt: String, config: Option<String>, styles: Option<String>) -> bool {
        const COMMAND: &str = "rofi";
        const YESNO: (&str, &str) = ("Yes", "No");
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
            Some(node)
        } else {
            match node
                .nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .find(|&n| n.id == node.focus[0])
            {
                Some(child) => find_focused(child),
                None => None,
            }
        }
    }
}

pub mod formatter {
    use i3ipc::reply::{Node, NodeLayout, NodeType, WindowProperty};
    use ignore::WalkBuilder;
    use regex::Regex;
    use serde::Serialize;
    use std::{collections::HashMap, path::PathBuf, str::FromStr};
    use tinytemplate::{format_unescaped, TinyTemplate};

    #[derive(Debug)]
    pub enum SmartGapsOption {
        Off,
        On,
        InverseOuter,
    }
    impl FromStr for SmartGapsOption {
        type Err = &'static str;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "0" => Ok(SmartGapsOption::Off),
                "1" => Ok(SmartGapsOption::On),
                "2" => Ok(SmartGapsOption::InverseOuter),
                _ => Err("no match"),
            }
        }
    }

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

    fn get_nodes_info(node: &Node) -> Vec<NodeInfo> {
        fn get_icon_by_class(class: &String, icon_map: &mut HashMap<String, String>) -> String {
            if let Some(icon_name) = icon_map.get(class) {
                icon_name.into()
            } else {
                let re_desktop =
                    Regex::new(format!(r"(?i:{}).*\.desktop$", class).as_str()).unwrap();
                let desktop_entries: Vec<String> = WalkBuilder::new("/usr/share/applications")
                    .build()
                    .filter_map(|entry| match entry {
                        Ok(e) => {
                            if e.path().is_file() {
                                if let Some(path) = e.path().to_str() {
                                    if re_desktop.is_match(path) {
                                        return Some(path.into());
                                    }
                                }
                            }
                            return None;
                        }
                        Err(_) => None,
                    })
                    .collect();
                let icon_name = match desktop_entries.get(0) {
                    Some(desktop_entry) => {
                        let re_desktop_icon = Regex::new(r"^Icon=").unwrap();
                        let text = std::fs::read_to_string(desktop_entry).unwrap();
                        match text.lines().find(|l| re_desktop_icon.is_match(l)) {
                            Some(icon_line) => {
                                match icon_line.split("=").collect::<Vec<&str>>().get(1) {
                                    Some(icon_name) => icon_name.trim().to_string(),
                                    None => class.clone(),
                                }
                            }
                            None => class.clone(),
                        }
                    }
                    None => class.clone(),
                };
                icon_map.insert(class.clone(), icon_name.clone());
                icon_name
            }
        }
        fn build_nodes_info(node: &Node, icon_map: &mut HashMap<String, String>) -> Vec<NodeInfo> {
            let mut nodes_info: Vec<NodeInfo> = Vec::new();
            if let Some(window_properties) = &node.window_properties {
                let class = window_properties
                    .get(&WindowProperty::Class)
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let title = window_properties
                    .get(&WindowProperty::Title)
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let icon = get_icon_by_class(&class, icon_map);
                nodes_info.push(NodeInfo { class, title, icon });
            }
            node.nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .for_each(|node| nodes_info.append(build_nodes_info(node, icon_map).as_mut()));
            nodes_info
        }
        let mut icon_map: HashMap<String, String> = HashMap::new();
        build_nodes_info(node, &mut icon_map)
    }

    fn find_inherited_rect(
        target: &Node,
        node: &Node,
        global_smart_gaps: SmartGapsOption,
        global_outer_gap: Option<i32>,
    ) -> NodeRect {
        fn _get_node_parent<'a>(target: &Node, node: &'a Node) -> Option<&'a Node> {
            match node
                .nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .find(|n| n.id == target.id)
            {
                Some(_) => Some(node),
                None => node
                    .nodes
                    .iter()
                    .chain(node.floating_nodes.iter())
                    .find(|n| _get_node_parent(target, n).is_some()),
            }
        }
        fn get_node_workspace<'a>(target: &Node, node: &'a Node) -> Option<&'a Node> {
            let workspaces = get_workspaces(node);
            match get_workspaces(node).iter().find(|w| w.id == target.id) {
                Some(w) => Some(w),
                None => workspaces
                    .iter()
                    .map(|w| *w)
                    .find(|w| is_node_descendant(target, w)),
            }
        }
        fn get_workspaces<'a>(node: &'a Node) -> Vec<&'a Node> {
            if node.nodetype == NodeType::Workspace {
                vec![node]
            } else {
                let mut workspaces: Vec<&'a Node> = Vec::new();
                node.nodes
                    .iter()
                    .chain(node.floating_nodes.iter())
                    .for_each(|w| {
                        workspaces.append(&mut get_workspaces(w));
                    });
                workspaces
            }
        }
        fn is_node_descendant(target: &Node, node: &Node) -> bool {
            match node
                .nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .find(|n| n.id == target.id)
            {
                Some(_) => true,
                None => node
                    .nodes
                    .iter()
                    .chain(node.floating_nodes.iter())
                    .find(|n| is_node_descendant(target, n))
                    .is_some(),
            }
        }
        fn can_workspace_of_node_have_gaps(target: &Node, node: &Node) -> Option<bool> {
            match get_node_workspace(target, node) {
                Some(workspace) => {
                    match workspace
                        .nodes
                        .iter()
                        .chain(workspace.floating_nodes.iter())
                        .find(|&n| n.id == workspace.focus[0])
                    {
                        Some(child) => {
                            if workspace.nodes.len() + workspace.floating_nodes.len() > 1 {
                                Some(true)
                            } else {
                                Some(
                                    child.layout != NodeLayout::Stacked
                                        && child.layout != NodeLayout::Tabbed,
                                )
                            }
                        }
                        None => None,
                    }
                }
                None => None,
            }
        }
        fn get_node_rect(node: &Node, with_gaps: bool, global_outer_gap: Option<i32>) -> NodeRect {
            let mut x = node.rect.0;
            let mut y = node.rect.1;
            let mut width = node.rect.2;
            let mut height = node.rect.3;
            if with_gaps {
                if let Some(gaps) = node.gaps {
                    x += gaps.left;
                    y += gaps.top;
                    width -= gaps.left + gaps.right;
                    height -= gaps.top + gaps.bottom;
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
                right: width + x,
                bottom: height + y,
                left: x,
            }
        }
        fn get_node_chain<'a>(target: &Node, node: &'a Node) -> Option<Vec<&'a Node>> {
            if node.id == target.id {
                return Some(vec![node]);
            }
            node.nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .find_map(|n| match get_node_chain(target, n) {
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
        let with_gaps = match can_workspace_of_node_have_gaps(target, node) {
            Some(can_have_gaps) => match global_smart_gaps {
                SmartGapsOption::Off => false,
                SmartGapsOption::On => {
                    if can_have_gaps {
                        true
                    } else {
                        false
                    }
                }
                SmartGapsOption::InverseOuter => {
                    if can_have_gaps {
                        false
                    } else {
                        true
                    }
                }
            },
            None => false,
        };
        let node_rect_default = get_node_rect(target, with_gaps, global_outer_gap);
        match get_node_chain(target, node) {
            Some(chain) => chain
                .iter()
                .rev()
                .map(|n| get_node_rect(n, with_gaps, global_outer_gap))
                .reduce(|a, b| NodeRect {
                    top: a.top.max(b.top),
                    right: a.right.min(b.right),
                    bottom: a.bottom.min(b.bottom),
                    left: a.left.max(b.left),
                })
                .unwrap_or(node_rect_default),
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
    ) -> (String, Option<String>) {
        let nodes_info = get_nodes_info(node);
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

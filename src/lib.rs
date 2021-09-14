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
    use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
    use i3ipc::reply::{Node, NodeLayout, NodeType, WindowProperty};
    use ignore::WalkBuilder;
    use regex::Regex;
    use serde::Serialize;
    use std::{
        collections::HashMap,
        env,
        fs::{self, OpenOptions},
        io::Write,
        path::PathBuf,
        str::FromStr,
    };
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
        fn get_icon_cache() -> Result<PathBuf, String> {
            let sub_dir = "i3-window-killer";
            let cache_file = "icons";
            let default_cache = ".cache";
            let directory = match env::var_os("XDG_CACHE_HOME") {
                Some(p_os_str) => match p_os_str.into_string() {
                    Ok(p_str) => Some(PathBuf::from(p_str).join(sub_dir)),
                    Err(_) => None,
                },
                None => match env::var_os("HOME") {
                    Some(p_os_str) => match p_os_str.into_string() {
                        Ok(p_str) => Some(PathBuf::from(p_str).join(default_cache).join(sub_dir)),
                        Err(_) => None,
                    },
                    None => None,
                },
            };
            match directory {
                Some(path) => {
                    let cache_file_path = path.join(cache_file);
                    if path.exists() {
                        Ok(cache_file_path)
                    } else {
                        match fs::create_dir_all(path) {
                            Ok(_) => Ok(cache_file_path),
                            Err(_) => Err(format!(
                                "couldn't create cache directory for {:#?}",
                                cache_file_path
                            )),
                        }
                    }
                }
                None => Err(format!("couldn't determine the user cache directory")),
            }
        }
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
                        Ok(text) => text.lines().find_map(|l| {
                            if l.starts_with(class) {
                                l.rsplit_once("=").map(|splits| splits.1.into())
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
                let name_line_key = "Name=";
                WalkBuilder::new("/usr/share/applications")
                    .build()
                    .filter_map(|entry| match entry {
                        Ok(e) => {
                            if e.path().is_file() {
                                if let Some(path) = e.path().to_str() {
                                    if re_desktop.is_match(path) {
                                        if let Ok(text) = fs::read_to_string(path) {
                                            text.lines().find_map(|l| {
                                                if l.starts_with(name_line_key) {
                                                    if matcher
                                                        .fuzzy_match(
                                                            l,
                                                            format!("{}{}", name_line_key, class)
                                                                .as_str(),
                                                        )
                                                        .is_some()
                                                    {
                                                        Some(path.into())
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
                    .collect::<Vec<String>>()
                    .get(0)
                    .map(|s| s.into())
            }
            if let Some(icon_name) = icon_map.get(class) {
                icon_name.into()
            } else {
                let icon_name = match get_icon_by_class_from_cache(class, cache_path) {
                    Some(icon_name) => icon_name,
                    None => {
                        let default_icon_name = class.clone();
                        let new_icon_name = match get_desktop_file_by_class(class) {
                            Some(desktop_entry) => {
                                if let Ok(text) = fs::read_to_string(desktop_entry) {
                                    let icon_line_key = "Icon=";
                                    match text.lines().find(|l| l.starts_with(icon_line_key)) {
                                        Some(icon_line) => {
                                            match icon_line.split("=").collect::<Vec<&str>>().get(1)
                                            {
                                                Some(icon_name) => icon_name.trim().to_string(),
                                                None => default_icon_name,
                                            }
                                        }
                                        None => default_icon_name,
                                    }
                                } else {
                                    default_icon_name
                                }
                            }
                            None => default_icon_name,
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
                    .get(&WindowProperty::Class)
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let title = window_properties
                    .get(&WindowProperty::Title)
                    .unwrap_or(&String::from("Unknown"))
                    .clone();
                let icon = get_icon_by_class(&class, cache_path, icon_map);
                nodes_info.push(NodeInfo { class, title, icon });
            }
            node.nodes
                .iter()
                .chain(node.floating_nodes.iter())
                .for_each(|node| {
                    nodes_info.append(build_nodes_info(node, cache_path, icon_map).as_mut())
                });
            nodes_info
        }
        let mut icon_map: HashMap<String, String> = HashMap::new();
        let cache_path = match get_icon_cache() {
            Ok(path) => Some(path),
            Err(e) => {
                eprintln!("error getting icon cache: {}", e);
                None
            }
        };
        build_nodes_info(node, &cache_path, &mut icon_map)
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
        fn can_workspace_of_node_have_gaps(
            target: &Node,
            node: &Node,
            smart_gaps: SmartGapsOption,
        ) -> Option<bool> {
            match get_node_workspace(target, node) {
                Some(workspace) => {
                    match workspace
                        .nodes
                        .iter()
                        .chain(workspace.floating_nodes.iter())
                        .find(|&n| n.id == workspace.focus[0])
                    {
                        Some(child) => {
                            let has_single_child =
                                workspace.nodes.len() + workspace.floating_nodes.len() == 1;
                            let has_gapless_layout = child.layout == NodeLayout::Stacked
                                || child.layout == NodeLayout::Tabbed;
                            match smart_gaps {
                                SmartGapsOption::Off => Some(false),
                                SmartGapsOption::On => {
                                    if has_single_child {
                                        Some(false)
                                    } else {
                                        if has_gapless_layout {
                                            Some(false)
                                        } else {
                                            Some(true)
                                        }
                                    }
                                }
                                SmartGapsOption::InverseOuter => {
                                    if has_single_child {
                                        Some(true)
                                    } else {
                                        Some(false)
                                    }
                                }
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
        let with_gaps = match can_workspace_of_node_have_gaps(target, node, global_smart_gaps) {
            Some(can_have_gaps) => can_have_gaps,
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

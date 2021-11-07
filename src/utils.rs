pub mod fs {
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    pub fn file_exists(path: String) -> Result<(), String> {
        if Path::new(&path).is_file() {
            Ok(())
        } else {
            Err(format!("{} is not a file", path))
        }
    }

    pub fn dir_exists(path: String) -> Result<(), String> {
        if Path::new(&path).is_dir() {
            Ok(())
        } else {
            Err(format!("{} is not a directory", path))
        }
    }

    pub fn create_parent_dir(path: &PathBuf) -> Result<(), std::io::Error> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    pub fn get_default_icon_cache() -> Option<String> {
        match env::var_os("XDG_CACHE_HOME") {
            Some(p_os_str) => p_os_str.into_string().ok(),
            None => match env::var_os("HOME") {
                Some(p_os_str) => {
                    if let Ok(p_str) = p_os_str.into_string() {
                        PathBuf::from(p_str)
                            .join(".cache")
                            .to_str()
                            .map(|s| s.to_owned())
                    } else {
                        None
                    }
                }
                None => None,
            },
        }
    }
}

pub mod i3_tree {
    use i3_ipc::reply::Node;

    pub fn get_child_iter<'a>(
        node: &'a Node,
    ) -> std::iter::Chain<std::slice::Iter<'a, Node>, std::slice::Iter<'a, Node>> {
        node.nodes.iter().chain(node.floating_nodes.iter())
    }

    pub fn find_focused(node: &Node) -> Option<&Node> {
        if node.focused {
            Some(node)
        } else {
            match get_child_iter(node).find(|&n| n.id == node.focus[0]) {
                Some(child) => find_focused(child),
                None => None,
            }
        }
    }
}

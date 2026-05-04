use serde::Deserialize;
use std::process::Command;

use super::{WindowInfo, WindowManager};

pub struct SwayManager;

impl SwayManager {
    pub fn new() -> Option<Self> {
        which::which("swaymsg").ok().map(|_| Self)
    }
}

impl WindowManager for SwayManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        let output = Command::new("swaymsg")
            .args(["-t", "get_tree", "-r"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let tree: SwayTree = serde_json::from_slice(&output.stdout).ok()?;

        fn find_focused(node: &SwayNode) -> Option<WindowInfo> {
            if node.focused {
                return Some(WindowInfo {
                    app_name: node
                        .app_id
                        .clone()
                        .or_else(|| node.name.clone())
                        .unwrap_or_default(),
                    window_title: node.name.clone().unwrap_or_default(),
                    class_name: node.app_id.clone().unwrap_or_default(),
                    focused: true,
                });
            }
            for child in &node.nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
            for child in &node.floating_nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
            None
        }

        find_focused(&tree.root)
    }

    fn name(&self) -> &'static str {
        "sway"
    }
}

#[derive(Debug, Deserialize)]
struct SwayTree {
    root: SwayNode,
}

#[derive(Debug, Deserialize)]
struct SwayNode {
    name: Option<String>,
    app_id: Option<String>,
    focused: bool,
    nodes: Vec<SwayNode>,
    #[serde(alias = "floating_nodes")]
    floating_nodes: Vec<SwayNode>,
}

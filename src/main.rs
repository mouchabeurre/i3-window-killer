use clap::{crate_version, App, Arg};
use i3_window_killer::{external_command::*, formatter::*, ipc_call::*, parser::*};
use i3ipc::{reply::NodeType, I3Connection};
use std::path::{Path, PathBuf};

const GLOBAL_OUTER_GAP: &str = "global_outer_gaps";
const SMART_GAPS: &str = "smart_gaps";
const ROFI_CONFIG: &str = "rofi_config";
const ROFI_THEME_FILE: &str = "rofi_theme_file";
const DUMP_STYLES: &str = "dump_styles";

#[derive(Debug)]
struct Options {
    global_outer_gap: Option<i32>,
    global_smart_gaps: SmartGapsOption,
    rofi_config: Option<String>,
    rofi_theme_file: Option<PathBuf>,
    dump_styles: bool,
}

fn file_exists(file_path: String) -> Result<(), String> {
    if Path::new(&file_path).is_file() {
        Ok(())
    } else {
        Err(format!("{} is not a file", file_path))
    }
}

fn main() {
    let matches = App::new("i3-window-killer")
        .version(crate_version!())
        .about("Show rofi confirmation prompt before killing the focused i3wm node")
        .arg(
            Arg::with_name(ROFI_CONFIG)
                .value_name("FILE")
                .long("config")
                .short("c")
                .help("rofi configuration file (passed as-is to subcommand)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(ROFI_THEME_FILE)
                .value_name("FILE")
                .long("template")
                .short("t")
                .help(
r##"rofi styles template
Template engine syntax: https://docs.rs/tinytemplate/1.2.1/tinytemplate/syntax/index.html
Interpolated variables:
    - container // Object of the focused node (access props with { container.prop })
        {
            top: Integer, // top value of the container rect in pixels
            right: Integer, 
            bottom: Integer,
            left: Integer,
        }
    - nodes // Array of windows within the container node
        [
            { // Object containing node props
                class: String, // window X11 class
                title: String, // window X11 title
                icon: String, // window desktop icon
            }
        ]"##,
                )
                .next_line_help(true)
                .takes_value(true)
                .validator(file_exists),
        )
        .arg(
            Arg::with_name(GLOBAL_OUTER_GAP)
                .value_name("INTEGER")
                .long("outer-gap")
                .short("o")
                .help(
r##"global i3-gaps "gaps outer" rule (in pixels)
If present in your i3 config, every node inherits the offset but their gaps property does not reflect it, so this hint helps in calculating the effective gaps."##,
                )
                .next_line_help(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name(SMART_GAPS)
                .value_name("INTEGER")
                .long("smart-gaps")
                .short("s")
                .help(
r##"global i3-gaps "smart_gaps" rule (0: off, 1: on, 2: inverse_outer)"##,
                )
                .takes_value(true)
                .possible_values(&["0","1","2"])
                .default_value("0")
        )
        .arg(
            Arg::with_name(DUMP_STYLES)
                .long("dump-styles")
                .short("d")
                .help(
r##"dump the rendered styles to stdout"##,
                )
        )
        .get_matches();
    let options = Options {
        dump_styles: matches.is_present(DUMP_STYLES),
        global_smart_gaps: matches
            .value_of(SMART_GAPS)
            .map(|s| {
                s.parse::<SmartGapsOption>()
                    .expect("couldn't parse smart_gap value")
            })
            .expect("couldn't get smart_gap option"),
        global_outer_gap: matches
            .value_of(GLOBAL_OUTER_GAP)
            .map(|s| s.parse::<i32>().expect("couldn't parse outer gap value")),
        rofi_config: matches.value_of(ROFI_CONFIG).map(|s| s.to_string()),
        rofi_theme_file: matches.value_of(ROFI_THEME_FILE).map(|s| PathBuf::from(s)),
    };
    let mut connection = I3Connection::connect().expect("failed to connect");
    let tree = get_tree(&mut connection).expect("failed to send command");
    let node = find_focused(&tree).expect("failed to find focused node");
    if node.nodetype == NodeType::Workspace && node.nodes.len() + node.floating_nodes.len() == 0 {
        return;
    }
    let (prompt, styles) = get_prompt_and_styles(
        &node,
        &tree,
        options.rofi_theme_file,
        options.global_smart_gaps,
        options.global_outer_gap,
    );
    if options.dump_styles {
        if let Some(ref styles) = styles {
            println!("{}", styles);
        }
    }
    if prompt_user(prompt, options.rofi_config, styles) {
        let outcomes = kill(&mut connection)
            .expect("failed to execute command")
            .outcomes;
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

use clap::{crate_version, App, Arg};
use i3_window_killer::{
    external_command::prompt_user,
    formatter::{get_prompt_and_styles, SmartGapsOption},
    ipc_call::{get_tree, kill},
    parser::find_focused,
    utils::{create_parent_dir, dir_exists, file_exists, get_default_icon_cache},
};
use i3ipc::{reply::NodeType, I3Connection};
use std::path::PathBuf;

const APP_NAME: &str = "i3-window-killer";
const ICONS_CACHE_FILENAME: &str = "icons";

const ARG_GLOBAL_OUTER_GAP: &str = "global_outer_gaps";
const ARG_SMART_GAPS: &str = "smart_gaps";
const ARG_ROFI_CONFIG: &str = "rofi_config";
const ARG_ROFI_THEME_FILE: &str = "rofi_theme_file";
const ARG_DUMP_STYLES: &str = "dump_styles";
const ARG_NO_CACHE: &str = "no_cache";
const ARG_CACHE_DIR: &str = "cache_dir";

#[derive(Debug)]
struct Options {
    global_outer_gap: Option<i32>,
    global_smart_gaps: SmartGapsOption,
    rofi_config: Option<String>,
    rofi_theme_file: Option<PathBuf>,
    dump_styles: bool,
    no_cache: bool,
    cache_file_path: Option<PathBuf>,
}

fn main() {
    let default_cache = get_default_icon_cache().unwrap_or_default();
    let matches = App::new(APP_NAME)
        .version(crate_version!())
        .about("Show rofi confirmation prompt before killing the focused i3wm node")
        .arg(
            Arg::with_name(ARG_ROFI_CONFIG)
                .value_name("FILE")
                .long("config")
                .short("c")
                .long_help("rofi configuration file (passed as-is to subcommand)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(ARG_ROFI_THEME_FILE)
                .value_name("FILE")
                .long("template")
                .short("t")
                .long_help(
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
        ]"##)
                .takes_value(true)
                .validator(file_exists),
        )
        .arg(
            Arg::with_name(ARG_GLOBAL_OUTER_GAP)
                .value_name("INTEGER")
                .long("outer-gap")
                .short("o")
                .long_help(
r##"Global i3-gaps "gaps outer" rule (in pixels)
If present in your i3 config, every node inherits the offset but their gaps property does not reflect it, so this hint helps in calculating the effective gaps."##)
                .takes_value(true),
        )
        .arg(
            Arg::with_name(ARG_SMART_GAPS)
                .value_name("INTEGER")
                .long("smart-gaps")
                .short("s")
                .long_help(
r##"Global i3-gaps "smart_gaps" rule (0: off, 1: on, 2: inverse_outer)"##,
                )
                .takes_value(true)
                .possible_values(&["0","1","2"])
                .hide_possible_values(true)
                .default_value("1")
        )
        .arg(
            Arg::with_name(ARG_DUMP_STYLES)
                .long("dump-styles")
                .short("d")
                .long_help("Dump rendered styles to stdout")
        )
        .arg(
            Arg::with_name(ARG_NO_CACHE)
                .long("no-cache")
                .long_help("Don't read/write cached icons")
                .conflicts_with(ARG_CACHE_DIR)
        )
        .arg(
            Arg::with_name(ARG_CACHE_DIR)
                .value_name("DIR")
                .long("cache-dir")
                .long_help(format!(
r##"Custom cache directory to use (sub-directory [{}] will still be created).
If unspecified, $XDG_CACHE_HOME or $HOME/.cache will be used"##, APP_NAME).as_str())
                .takes_value(true)
                .validator(dir_exists)
                .default_value(default_cache.as_str()),
        )
        .get_matches();

    let options = Options {
        no_cache: matches.is_present(ARG_NO_CACHE),
        cache_file_path: matches
            .value_of(ARG_CACHE_DIR)
            .map(|s| {
                if s.is_empty() || matches.is_present(ARG_NO_CACHE) {
                    None
                } else {
                    Some(PathBuf::from(s).join(APP_NAME).join(ICONS_CACHE_FILENAME))
                }
            })
            .unwrap_or(None),
        dump_styles: matches.is_present(ARG_DUMP_STYLES),
        global_smart_gaps: matches
            .value_of(ARG_SMART_GAPS)
            .map(|s| {
                s.parse::<SmartGapsOption>()
                    .expect("couldn't parse smart_gap value")
            })
            .expect("couldn't get smart_gap option"),
        global_outer_gap: matches
            .value_of(ARG_GLOBAL_OUTER_GAP)
            .map(|s| s.parse::<i32>().expect("couldn't parse outer gap value")),
        rofi_config: matches.value_of(ARG_ROFI_CONFIG).map(|s| s.to_string()),
        rofi_theme_file: matches
            .value_of(ARG_ROFI_THEME_FILE)
            .map(|s| PathBuf::from(s)),
    };

    if let Some(ref path) = options.cache_file_path {
        if let Err(error) = create_parent_dir(path) {
            eprintln!("Couldn't create cache directory: {}", error)
        }
    }

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
        options.cache_file_path,
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

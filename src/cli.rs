use crate::utils::fs::{dir_exists, file_exists, get_default_icon_cache};
use clap::{crate_version, App, Arg};
use std::{path::PathBuf, str::FromStr};

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

#[derive(Debug)]
pub struct Options {
    pub global_outer_gap: Option<i32>,
    pub global_smart_gaps: SmartGapsOption,
    pub rofi_config: Option<String>,
    pub rofi_theme_file: Option<PathBuf>,
    pub dump_styles: bool,
    pub no_cache: bool,
    pub cache_file_path: Option<PathBuf>,
}

pub fn get_options() -> Options {
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

    Options {
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
    }
}

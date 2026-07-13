use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DebugUi {
    pub component_border: bool,
    pub render_profile: bool,
}

#[derive(Debug, Clone)]
pub struct GameplayConfig {
    pub music_assets_path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub ui: DebugUi,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub gameplay: GameplayConfig,
    pub debug: DebugConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            gameplay: GameplayConfig {
                music_assets_path: vec!["assets/builtin_musics".to_string()],
            },
            debug: DebugConfig {
                ui: DebugUi {
                    component_border: false,
                    render_profile: true,
                },
            },
        }
    }
}

fn resolve_config_path() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "-c" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "config.example.toml".to_string()
}

pub fn load_config() -> Config {
    let path_str = resolve_config_path();
    let path = Path::new(&path_str);
    if !path.exists() {
        return Config::default();
    }
    match fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<toml::Table>(&content) {
            Ok(value) => {
                let gameplay = GameplayConfig {
                    music_assets_path: value
                        .get("gameplay")
                        .and_then(|g| g.get("music_assets_path"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_else(|| vec!["assets/builtin_musics".to_string()]),
                };
                let component_border = value
                    .get("debug")
                    .and_then(|d| d.get("ui"))
                    .and_then(|u| u.get("component_border"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let render_profile = value
                    .get("debug")
                    .and_then(|d| d.get("ui"))
                    .and_then(|u| u.get("render_profile"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                Config {
                    gameplay,
                    debug: DebugConfig {
                        ui: DebugUi {
                            component_border,
                            render_profile,
                        },
                    },
                }
            }
            Err(e) => {
                eprintln!("load_config: TOML parse error: {:?}", e);
                Config::default()
            }
        },
        Err(e) => {
            eprintln!("load_config: file read error: {:?}", e);
            Config::default()
        }
    }
}

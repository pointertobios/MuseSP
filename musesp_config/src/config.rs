use std::path::Path;

#[derive(Debug, Clone)]
pub struct DebugUi {
    pub component_border: bool,
    pub render_profile: bool,
}

#[derive(Debug, Clone)]
pub struct DebugGameplay {
    pub camera_adjustable: bool,
}

#[derive(Debug, Clone)]
pub struct CameraConfig {
    pub eye: [f32; 3],
    /// 摄像机朝向（单位向量）
    pub direction: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            eye: [35.0, 25.0, 35.0],
            direction: [-0.631, -0.451, -0.631],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 60.0,
            near: 0.1,
            far: 200.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameplayConfig {
    pub music_assets_path: Vec<String>,
    /// 音符飞行速度（单位/秒），音符从原点飞到 r=20 球面耗时 20/notespeed 秒
    pub notespeed: f32,
    /// 摄像机初始参数
    pub camera: CameraConfig,
}

#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub ui: DebugUi,
    pub gameplay: DebugGameplay,
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
                notespeed: 0.0,
                camera: CameraConfig::default(),
            },
            debug: DebugConfig {
                ui: DebugUi {
                    component_border: false,
                    render_profile: true,
                },
                gameplay: DebugGameplay {
                    camera_adjustable: true,
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

fn parse_camera_config(value: &toml::Table) -> CameraConfig {
    let default = CameraConfig::default();
    let camera_table = match value.get("gameplay").and_then(|g| g.get("camera")) {
        Some(toml::Value::Table(t)) => t,
        _ => return default,
    };

    let parse_f3 = |key: &str, fallback: [f32; 3]| -> [f32; 3] {
        camera_table
            .get(key)
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                if arr.len() == 3 {
                    Some([
                        arr[0].as_float().or_else(|| arr[0].as_integer().map(|i| i as f64))? as f32,
                        arr[1].as_float().or_else(|| arr[1].as_integer().map(|i| i as f64))? as f32,
                        arr[2].as_float().or_else(|| arr[2].as_integer().map(|i| i as f64))? as f32,
                    ])
                } else {
                    None
                }
            })
            .unwrap_or(fallback)
    };

    CameraConfig {
        eye: parse_f3("eye", default.eye),
        direction: parse_f3("direction", default.direction),
        up: parse_f3("up", default.up),
        fov_degrees: camera_table
            .get("fov_degrees")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(default.fov_degrees as f64) as f32,
        near: camera_table
            .get("near")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(default.near as f64) as f32,
        far: camera_table
            .get("far")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .unwrap_or(default.far as f64) as f32,
    }
}

/// 将摄像机参数写回配置文件
pub async fn save_camera_config(camera: &CameraConfig) {
    let path_str = resolve_config_path();
    let path = Path::new(&path_str);

    let mut table = match tokio::fs::read_to_string(path).await {
        Ok(content) => toml::from_str::<toml::Table>(&content).unwrap_or_default(),
        Err(_) => toml::Table::new(),
    };

    let f3 = |v: [f32; 3]| -> toml::Value {
        toml::Value::Array(vec![
            toml::Value::Float(v[0] as f64),
            toml::Value::Float(v[1] as f64),
            toml::Value::Float(v[2] as f64),
        ])
    };

    let mut cam_table = toml::Table::new();
    cam_table.insert("eye".into(), f3(camera.eye));
    cam_table.insert("direction".into(), f3(camera.direction));
    cam_table.insert("up".into(), f3(camera.up));
    cam_table.insert(
        "fov_degrees".into(),
        toml::Value::Float(camera.fov_degrees as f64),
    );
    cam_table.insert("near".into(), toml::Value::Float(camera.near as f64));
    cam_table.insert("far".into(), toml::Value::Float(camera.far as f64));

    let gameplay = table
        .entry("gameplay".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let Some(g) = gameplay.as_table_mut() {
        g.insert("camera".into(), toml::Value::Table(cam_table));
    }

    if let Ok(content) = toml::to_string(&table) {
        let _ = tokio::fs::write(path, content).await;
    }
}

/// 同步版本：在 tokio runtime 不可用时使用（如 Drop 中）
pub fn save_camera_config_sync(camera: &CameraConfig) {
    let path_str = resolve_config_path();
    let path = Path::new(&path_str);

    let mut table = match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str::<toml::Table>(&content).unwrap_or_default(),
        Err(_) => toml::Table::new(),
    };

    let f3 = |v: [f32; 3]| -> toml::Value {
        toml::Value::Array(vec![
            toml::Value::Float(v[0] as f64),
            toml::Value::Float(v[1] as f64),
            toml::Value::Float(v[2] as f64),
        ])
    };

    let mut cam_table = toml::Table::new();
    cam_table.insert("eye".into(), f3(camera.eye));
    cam_table.insert("direction".into(), f3(camera.direction));
    cam_table.insert("up".into(), f3(camera.up));
    cam_table.insert(
        "fov_degrees".into(),
        toml::Value::Float(camera.fov_degrees as f64),
    );
    cam_table.insert("near".into(), toml::Value::Float(camera.near as f64));
    cam_table.insert("far".into(), toml::Value::Float(camera.far as f64));

    let gameplay = table
        .entry("gameplay".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let Some(g) = gameplay.as_table_mut() {
        g.insert("camera".into(), toml::Value::Table(cam_table));
    }

    if let Ok(content) = toml::to_string(&table) {
        let _ = std::fs::write(path, content);
    }
}

pub async fn load_config() -> Config {
    let path_str = resolve_config_path();
    let path = Path::new(&path_str);
    if tokio::fs::metadata(path).await.is_err() {
        return Config::default();
    }
    match tokio::fs::read_to_string(path).await {
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
                    notespeed: value
                        .get("gameplay")
                        .and_then(|g| g.get("notespeed"))
                        .and_then(|v| {
                            v.as_float()
                                .or_else(|| v.as_integer().map(|i| i as f64))
                        })
                        .unwrap_or_else(|| {
                            eprintln!(
                                "FATAL: gameplay.notespeed is required in config file (e.g. notespeed = 12)"
                            );
                            std::process::exit(1);
                        }) as f32,
                    camera: parse_camera_config(&value),
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
                let camera_adjustable = value
                    .get("debug")
                    .and_then(|d| d.get("gameplay"))
                    .and_then(|g| g.get("camera_adjustable"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                Config {
                    gameplay,
                    debug: DebugConfig {
                        ui: DebugUi {
                            component_border,
                            render_profile,
                        },
                        gameplay: DebugGameplay { camera_adjustable },
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

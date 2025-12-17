use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use unshell_lib::debug;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ComponentMetadata {
    name: String,
    description: Option<String>,
    version: Option<String>,
    authors: Option<Vec<String>>,

    // Struct to contain build information
    build_config: BuildConfig,

    // Other components that can be pointed to by this component
    #[serde(default)]
    child_components: Vec<PathBuf>,
    // config: Option<HashMap<String, ConfigStructField>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct BuildConfig {
    // Cargo feature list of a component
    // (Name, Description)
    #[serde(default)]
    features: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
enum ConfigStructField {
    Header(String),
    Text(String),
    String {
        // Default value of string edit in struct
        #[serde(default)]
        default: String,
        max_length: Option<usize>,
        // Display string edit as password
        #[serde(default)]
        protected: Option<bool>,
    },
    Integer {
        // Default value of integer in struct
        #[serde(default)]
        default: i32,
        min: Option<i32>,
        max: Option<i32>,
    },
    // Checkbox
    // Dropdown
    // Collapsing header
    // Slider
    // ...
}

pub fn load_config(path: &PathBuf) -> Result<Vec<ComponentMetadata>, Box<dyn Error>> {
    let path_absolute = fs::canonicalize(path.clone())?;
    debug!("Loading data from path: `{}`", path_absolute);

    // Read string as path
    let config_str = fs::read_to_string(path.clone())?;

    // Load config from String
    let config = toml::from_str::<ComponentMetadata>(&config_str)?;

    if config.child_components.is_empty() {
        Ok(vec![config])
    } else {
        let parent_path = path_absolute.parent().expect("Path must have parent");

        let mut config_vec = vec![];

        // Load each child component
        for component_path in &config.child_components {
            let path = Path::join(parent_path, component_path);
            let mut config = load_config(&path)?;
            config_vec.append(&mut config);
        }

        config_vec.insert(0, config);

        Ok(config_vec)
    }
}

// pub fn parse_toml() -> ComponentMetadata {
//     let data = include_str!("../../test.toml");

//     let config = toml::from_str(data).unwrap();

//     config
// }

mod blob;

use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use unshell_lib::{ModuleError, Result, debug, info};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct ComponentMetadata {
    name: String,
    description: Option<String>,
    version: Option<String>,
    authors: Option<Vec<String>>,

    // Struct to contain build information
    #[serde(default)]
    build_config: BuildConfig,

    // Other components that can be pointed to by this component
    #[serde(default)]
    child_components: Vec<PathBuf>,
}

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
struct BuildConfig {
    // Cargo feature list of a component
    // (Name, Description)
    #[serde(default)]
    features: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ComponentState {
    metadata: ComponentMetadata,
    path: PathBuf,
}

pub fn load_config(path: &PathBuf) -> Result<Vec<ComponentState>> {
    let path_absolute =
        fs::canonicalize(path.clone()).map_err(|e| ModuleError::Error(e.to_string()))?;
    debug!("Loading data from path: `{:?}`", path_absolute);

    // Read string as path
    let config_str =
        fs::read_to_string(path.clone()).map_err(|e| ModuleError::Error(e.to_string()))?;

    // Load config from String
    let config = toml::from_str::<ComponentMetadata>(&config_str)
        .map_err(|e| ModuleError::Error(e.to_string()))?;

    info!("Loaded component `{}`", config.name);

    let parent_path = path_absolute.parent().expect("Path must have parent");

    if config.child_components.is_empty() {
        Ok(vec![ComponentState {
            metadata: config,
            path: PathBuf::from(parent_path),
        }])
    } else {
        let mut config_vec = vec![];

        // Load each child component
        for component_path in &config.child_components {
            let path = Path::join(parent_path, component_path);
            let mut config = load_config(&path)?;
            config_vec.append(&mut config);
        }

        config_vec.insert(
            0,
            ComponentState {
                metadata: config,
                path: PathBuf::from(parent_path),
            },
        );

        Ok(config_vec)
    }
}

// pub fn parse_toml() -> ComponentMetadata {
//     let data = include_str!("../../test.toml");

//     let config = toml::from_str(data).unwrap();

//     config
// }

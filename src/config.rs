use crate::platform::{Model, Platform};
use crate::{e, s};
use std::collections::{HashMap, HashSet};
type Result<T> = result::Result<T, Box<dyn error::Error>>;
use std::*;
pub struct Models {
    config_filepath: String,
    models: HashMap<String, Model>,
}
impl Models {
    fn new(filepath: &str) -> Self {
        Models {
            config_filepath: filepath.to_string(),
            models: HashMap::new(),
        }
    }
    /// adds a model
    fn add(&mut self, key: String, model: Model) -> Result<()> {
        if !self.models.contains_key(&key) {
            self.models.insert(key, model);
        }
        Ok(())
    }
    /// removes a model
    fn remove(&mut self, key: &str) -> Result<()> {
        if let Some(model) = self.models.remove(key) {
            model.abort().map_err(s!())?;
            thread::spawn(move || drop(model));
        }
        Ok(())
    }
    /// checks each model, and starts download if online
    pub fn download(&mut self) -> Result<()> {
        for (_, model) in &mut self.models {
            model.download().map_err(s!())?;
            thread::sleep(time::Duration::from_millis(500));
        }
        Ok(())
    }
    /// updates Models struct with json
    pub fn update_config(&mut self) -> Result<()> {
        let mut new_models = match load(&self.config_filepath) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                return Ok(());
            }
        };
        let new = &mut new_models.models;
        // create hash sets
        let current_set = self.models.keys().cloned().collect::<HashSet<String>>();
        let new_set = new.keys().cloned().collect::<HashSet<String>>();
        // remove
        for key in current_set.difference(&new_set) {
            self.remove(key).map_err(s!())?;
        }
        // add
        for key in new_set.difference(&current_set) {
            if let Some(mut model) = new.remove(key) {
                model.download().map_err(s!())?;
                self.add(key.to_string(), model).map_err(s!())?;
            }
        }
        Ok(())
    }
}
/// Initializes Models struct by loading it with model names from cb-config.json
pub fn load(json_path: &str) -> Result<Models> {
    let mut models = Models::new(json_path);
    let json = parse_json(json_path).map_err(s!())?;
    if let Some(platforms) = json["platform"].as_object() {
        for (platform, usernames) in platforms {
            if let Some(p) = Platform::new(platform) {
                if let Some(usernames) = usernames.as_array() {
                    for username in usernames {
                        if let Some(username) = username.as_str() {
                            let model = Model::new(p.clone(), username);
                            models.add(model.composite_key(), model).map_err(s!())?;
                        }
                    }
                }
            }
        }
    }
    Ok(models)
}
/// parses json from cb-config.json as a serde data
fn parse_json(filepath: &str) -> Result<serde_json::Value> {
    let json_raw = match fs::read_to_string(filepath) {
        Ok(r) => r,
        Err(r) => {
            if r.kind() != io::ErrorKind::NotFound {
                return Err(r).map_err(e!())?;
            }
            let json = serde_json::json!({
                "platform": {
                    "CB": [],
                    "MFC": [],
                    "SCVR": [],
                    "SC": []
                }
            });
            fs::write(filepath, serde_json::to_string_pretty(&json).map_err(e!())?).map_err(e!())?;
            println!("Fill in {} with the given fields", filepath);
            process::exit(0)
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    Ok(json)
}

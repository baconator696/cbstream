use crate::cb;
use crate::{e, o, s};
use std::collections::HashMap;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
use std::*;
pub trait ModelInfo {
    fn is_online(&mut self) -> Result<bool>;
    fn is_finished(&self) -> bool;
    fn download(&mut self) -> Result<()>;
    fn clean_handle(&mut self) -> Result<()>;
    fn abort(&self) -> Result<()>;
}
pub struct Models {
    filepath: String,
    models: HashMap<String, HashMap<String, Box<dyn ModelInfo>>>,
}
impl Models {
    /// checks each model, and starts download if online
    pub fn download(&mut self) -> Result<()> {
        for (_, models) in &mut self.models {
            for (_, model) in models {
                if !model.is_finished() {
                    continue;
                }
                if model.is_online().map_err(s!())? {
                    model.download().map_err(s!())?;
                }
                thread::sleep(time::Duration::from_millis(500));
                if model.is_finished() {
                    model.clean_handle().map_err(s!())?;
                }
            }
        }
        self.update_config().map_err(s!())?;
        Ok(())
    }
    /// updates Models struct with json if the json was updated
    fn update_config(&mut self) -> Result<()> {
        let json = parse_json(&self.filepath).map_err(s!())?;
        // reloads CB models from json
        self.update_config_internal(&json, "CB models", cb::update).map_err(s!())?;
        Ok(())
    }
    fn update_config_internal(
        &mut self,
        json: &serde_json::Value,
        key: &str,
        update_func: fn(
            Option<&Vec<serde_json::Value>>,
            &mut HashMap<String, Box<dyn ModelInfo>>,
        ) -> HashMap<String, Box<dyn ModelInfo>>,
    ) -> Result<()> {
        let old_models = self.models.get_mut(key).ok_or_else(o!())?;
        let new_models = update_func(json[key].as_array(), old_models);
        for (name, model) in old_models {
            if !new_models.contains_key(name) {
                model.abort().map_err(s!())?;
                model.clean_handle().map_err(s!())?;
            }
        }
        self.models.insert(key.into(), new_models);
        Ok(())
    }
}
impl Drop for Models {
    /// waits on download handle finish, to allow the thread to mux the downloaded files
    fn drop(&mut self) {
        for (_, models) in &mut self.models {
            for (_, model) in models {
                if let Err(e) = model.clean_handle().map_err(s!()) {
                    eprintln!("{}", e);
                };
            }
        }
    }
}
/// Initializes Models struct by loading it with model names from cb-config.json
pub fn load(json_path: &str) -> Result<Models> {
    let json = parse_json(json_path).map_err(s!())?;
    let mut models: HashMap<String, HashMap<String, Box<dyn ModelInfo>>> = HashMap::new();
    // loads CB models from json
    let k = "CB models";
    let new = cb::new;
    models.insert(k.into(), new(json[k].as_array()));
    Ok(Models {
        filepath: json_path.to_string(),
        models,
    })
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
                "CB models": [],
                "MFC models": [],
                "SC models": []
            });
            fs::write(filepath, serde_json::to_string_pretty(&json).map_err(e!())?)
                .map_err(e!())?;
            return Err(format!("Fill in {} with the given fields", filepath))?;
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    Ok(json)
}
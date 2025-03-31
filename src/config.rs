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
        Ok(())
    }
    /// updates Models struct with json if the json was updated
    pub fn update_config(&mut self) -> Result<()> {
        let json = parse_json(&self.filepath).map_err(s!())?;
        // reloads CB models from json
        update_internal(&mut self.models, "CB models", &json, cb::Cb::new).map_err(s!())?;
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
    let (k, m) = load_internal(&json, "CB models", cb::Cb::new);
    models.insert(k, m);
    Ok(Models {
        filepath: json_path.to_string(),
        models,
    })
}
fn load_internal<T: ModelInfo + 'static>(
    models_json: &serde_json::Value,
    key: &str,
    func: fn(&str) -> T,
) -> (String, HashMap<String, Box<dyn ModelInfo>>) {
    let mut map: HashMap<String, Box<dyn ModelInfo>> = HashMap::new();
    if let Some(models) = models_json[key].as_array() {
        for model in models {
            if let Some(model) = model.as_str() {
                map.insert(model.to_string(), Box::new(func(model)));
            }
        }
    }
    (key.to_string(), map)
}
fn update_internal<T: ModelInfo + 'static>(
    models: &mut HashMap<String, HashMap<String, Box<dyn ModelInfo>>>,
    key: &str,
    models_json: &serde_json::Value,
    func: fn(&str) -> T,
) -> Result<()> {
    let old_models = models.get_mut(key).ok_or_else(o!())?;
    let mut new_models: HashMap<String, Box<dyn ModelInfo>> = HashMap::new();
    if let Some(models_json) = models_json[key].as_array() {
        for model in models_json {
            if let Some(model) = model.as_str() {
                if new_models.contains_key(model) {
                    continue;
                }
                if old_models.contains_key(model) {
                    new_models.insert(model.to_string(), old_models.remove(model).unwrap());
                } else {
                    new_models.insert(model.to_string(), Box::new(func(model)));
                }
            }
        }
    }
    for (name, model) in old_models {
        if !new_models.contains_key(name) {
            model.abort().map_err(s!())?;
            model.clean_handle().map_err(s!())?;
        }
    }
    models.insert(key.into(), new_models);
    Ok(())
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
            fs::write(filepath, serde_json::to_string_pretty(&json).map_err(e!())?).map_err(e!())?;
            println!("Fill in {} with the given fields", filepath);
            process::exit(0)
        }
    };
    let json: serde_json::Value = serde_json::from_str(&json_raw).map_err(e!())?;
    Ok(json)
}

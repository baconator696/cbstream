use {
    crate::{
        e, h,
        platforms::{Model, Platform},
        s,
    },
    std::{
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
        sync::Arc,
        *,
    },
};
type Res<T> = Result<T, Box<dyn error::Error>>;
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct Settings {
    #[serde(rename = "user-agent", default = "default_useragent")]
    pub user_agent: String,
}
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    #[serde(skip)]
    json_location: PathBuf,
    config: Settings,
    #[serde(deserialize_with = "deserialize_platform")]
    platform: HashMap<Platform, HashSet<Model>>,
}
pub fn init(filepath: &Path) -> Res<Config> {
    let config_json_data = match fs::read(filepath) {
        Ok(r) => r,
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                println!(
                    "configuration file created at {}\nplease fill with relatvent data",
                    filepath.display()
                );
                let config = Config::default(filepath);
                config.write().map_err(s!())?;
                return Ok(config);
            }
            return Err(e).map_err(e!())?;
        }
    };
    let mut config: Config = serde_json::from_slice(&config_json_data).map_err(e!())?;
    config.json_location = PathBuf::from(filepath);
    Ok(config)
}
impl Config {
    fn read(filepath: &Path) -> Res<Self> {
        let config_json_data = fs::read(filepath).map_err(e!())?;
        let mut config: Config = serde_json::from_slice(&config_json_data).map_err(e!())?;
        config.json_location = PathBuf::from(filepath);
        Ok(config)
    }
    fn write(&self) -> Res<()> {
        let json = serde_json::to_string_pretty(&self).map_err(e!())?;
        fs::write(&self.json_location, json).map_err(e!())?;
        Ok(())
    }
    /// adds a model
    fn model_add_unchecked_start(&mut self, mut model: Model) -> Res<()> {
        model
            .download(Arc::new(self.config.clone()))
            .map_err(s!())?;
        if let Some(platform) = self.platform.get_mut(&model.platform) {
            platform.insert(model);
        } else {
            let platform_key = model.platform.clone();
            let mut platform: HashSet<Model> = HashSet::new();
            platform.insert(model);
            self.platform.insert(platform_key, platform);
        }
        Ok(())
    }
    /// removes a model
    fn model_remove_stop(&mut self, model_ref: &Model) -> Res<()> {
        if let Some(platform) = self.platform.get_mut(&model_ref.platform) {
            if let Some(model) = platform.take(model_ref) {
                model.abort().map_err(s!())?;
                thread::spawn(move || drop(model));
            }
        }
        Ok(())
    }
    /// checks each model, and starts download if online
    pub fn download(&mut self) -> Res<()> {
        let mut headers: Vec<_> = Vec::new();
        let settings = Arc::new(self.config.clone());
        for (_, mut models) in self.platform.drain() {
            let s = settings.clone();
            let handle: thread::JoinHandle<Result<HashSet<Model>, String>> =
                thread::spawn(move || {
                    for model_ref in models.clone() {
                        if let Some(mut model) = models.take(&model_ref) {
                            model.download(s.clone()).map_err(s!())?;
                            models.insert(model);
                            thread::sleep(time::Duration::from_millis(500));
                        }
                    }
                    Ok(models)
                });
            headers.push(handle);
        }
        for header in headers {
            let models = header.join().map_err(h!())?.map_err(s!())?;
            if let Some(platform) = models.iter().next().map(|o| o.platform.clone()) {
                self.platform.insert(platform, models);
            }
        }
        Ok(())
    }
    /// updates Models struct with json
    pub fn update_config(&mut self) -> Res<()> {
        let new_config = match Self::read(&self.json_location).map_err(s!()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}", e);
                return Ok(());
            }
        };
        let current_map: HashMap<String, Model> = self
            .platform
            .values()
            .flat_map(|m| m.iter().map(|m| (m.composite_key(), (*m).clone())))
            .collect();
        let current_set: HashSet<&String> = current_map.keys().collect();
        let new_map: HashMap<String, &Model> = new_config
            .platform
            .values()
            .flat_map(|m| m.iter().map(|m| (m.composite_key(), m)))
            .collect();
        let new_set: HashSet<&String> = new_map.keys().collect();
        // remove
        for key in current_set.difference(&new_set) {
            if let Some(ref_model) = current_map.get(*key) {
                self.model_remove_stop(ref_model).map_err(s!())?
            }
        }
        // add
        for key in new_set.difference(&current_set) {
            if let Some(new_model) = new_map.get(*key) {
                self.model_add_unchecked_start((*new_model).clone())
                    .map_err(s!())?
            }
        }
        self.config = new_config.config;
        Ok(())
    }
    fn default(json_location: &Path) -> Self {
        let platform: HashMap<Platform, HashSet<Model>> = Platform::list()
            .into_iter()
            .map(|platform| (platform, HashSet::new()))
            .collect();
        Config {
            json_location: json_location.to_path_buf(),
            config: Settings {
                user_agent: String::new(),
            },
            platform,
        }
    }
}
fn default_useragent() -> String {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36 Edg/146.0.0.0".into()
}
fn deserialize_platform<'de, D>(
    deserializer: D,
) -> Result<HashMap<Platform, HashSet<Model>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let raw = HashMap::<String, Vec<String>>::deserialize(deserializer)?;
    let mut data: HashMap<Platform, HashSet<Model>> = HashMap::new();
    for (platform_str, model_strs) in raw {
        let platform_opt = Platform::new(&platform_str);
        if let Some(platform) = platform_opt {
            let models: HashSet<Model> = model_strs
                .iter()
                .map(|model| Model::new(platform.clone(), model))
                .collect();
            data.insert(platform, models);
        }
    }
    Ok(data)
}

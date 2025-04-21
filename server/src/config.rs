use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Setting {
    #[serde(default)]
    pub runtime: Runtime,
    pub ingester: Ingester,
    pub transmitter: Transmitter,
    pub cache: Cache,
}

#[derive(Debug, Default, Deserialize)]
pub struct Runtime {
    pub threads: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct Ingester {
    pub addr: String,
}

#[derive(Debug, Deserialize)]
pub struct Transmitter {
    pub addr: String,
}

pub enum CacheConfig {
    NotFound,
    Static(StaticCache),
    Map(MapCache),
    List(ListCache),
}

#[derive(Debug, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub map: Vec<MapCache>,
    #[serde(default)]
    pub list: Vec<ListCache>,
    #[serde(default)]
    pub r#static: Vec<StaticCache>,
}

impl Cache {
    pub fn config(&self, name: &str) -> CacheConfig {
        let parts = name.split(':').collect::<Vec<&str>>();
        if parts.len() != 2 {
            return CacheConfig::NotFound;
        }

        match parts[0] {
            "static" => {
                for c in self.r#static.iter() {
                    if c.name.eq(parts[1]) {
                        return CacheConfig::Static(c.clone());
                    }
                }
                CacheConfig::NotFound
            }
            "list" => {
                for c in self.list.iter() {
                    if c.name == parts[1] {
                        return CacheConfig::List(c.clone());
                    }
                }
                CacheConfig::NotFound
            }
            "map" => {
                for c in self.map.iter() {
                    if c.name == parts[1] {
                        return CacheConfig::Map(c.clone());
                    }
                }
                CacheConfig::NotFound
            }
            _ => CacheConfig::NotFound,
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct StaticCache {
    pub name: String,
    pub file_path: String,
    pub shards: u64,
    pub streams: u64,
    pub tracks: u64,
    pub segments: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapCache {
    pub name: String,
    pub preallocate: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListCache {
    pub name: String,
    pub copy: bool,
}

use std::path::PathBuf;
use rand::Rng;

pub struct YamlMockFile {
  pub path: PathBuf,
}


impl YamlMockFile {
  pub fn from_yaml(yaml: &str) -> Self {
    let random_string: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .collect();
    let path = PathBuf::from(format!("/tmp/{random_string}.yaml"));
    std::fs::write(&path, yaml).unwrap();
    Self { path }
  }
}

impl Drop for YamlMockFile {
  fn drop(&mut self) {
    std::fs::remove_file(&self.path).unwrap();
  }
}
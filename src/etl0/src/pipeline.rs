use std::fs::{read_dir, DirEntry, Metadata, ReadDir};
use std::path::{Path, PathBuf};
use std::slice::Iter;
use std::str::Lines;

use regex::Regex;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Debug)]
pub struct Semver {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Semver {
    fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch}
    }
}

#[derive(Debug)]
pub struct Pipeline {
    pub path: String,
    pub length: usize,
    tasks: Vec<Task>,
}

impl Pipeline {
    async fn open(path: PathBuf) -> Self {
        let mut file: File = match File::open(&path).await {
            Err(error) => panic!("{:?}", error),
            Ok(value) => value,
        };

        let mut content: String = String::with_capacity(10 * 1024);
        let length: usize = match file.read_to_string(&mut content).await {
            Err(error) => panic!("{:?}", error),
            Ok(value) => value,
        };

        let lines: Lines = content.lines();
        let path = match path.to_str() {
            None => panic!("{:?}", "path"),
            Some(value) => value.to_owned(),
        };

        Self {
            path: path,
            length: length,
            tasks: Task::read_all(lines),
        }
    }

    pub fn tasks(&self) -> Iter<'_, Task> {
        self.tasks.iter()
    }
}

#[derive(Debug)]
pub struct Task {
    pub line: usize,
    pub content: String,
    pub image: String,
    pub plugin: PluginRef,
}

impl Task {
    fn read_all(lines: Lines) -> Vec<Self> {
        let mut start = 0;
        let mut tasks: Vec<Self> = Vec::new();
        let mut meta = Vec::new();
        let mut content = Vec::new();

        for (index, line) in lines.enumerate() {
            if line.starts_with("``` ") {
                if content.len() > 0 {
                    tasks.push(Self::read(start, &meta, &content));
                    meta.clear();
                    content.clear();
                }

                if content.len() == 0 {
                    start = index;
                }

                meta.push(line);
            } else {
                content.push(line);
            }
        }

        if content.len() > 0 {
            tasks.push(Self::read(start, &meta, &content));
        }

        tasks
    }

    fn read(line: usize, meta: &[&str], content: &[&str]) -> Self {
        Self {
            line: line,
            content: content.join("\n"),
            image: "".to_owned(),
            plugin: Self::extract_plugin(meta),
        }
    }

    fn extract_plugin(meta: &[&str]) -> PluginRef {
        let vendor: &str = r"(?P<vendor>[a-zA-Z0-9]+)";
        let dep: &str = r"(?P<dep>[a-zA-Z0-9]+)";
        let semver: &str = r"((?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+))";
        let pattern: String = format!(r"^``` {vendor}/{dep}@{semver}$");

        let regex: Regex = match Regex::new(&pattern) {
            Err(error) => panic!("wrong regex {:?}", error),
            Ok(value) => value,
        };

        match meta.get(0) {
            None => todo!("missing index"),
            Some(value) => match regex.captures(value) {
                None => todo!("captures"),
                Some(captures) => {
                    let vendor: String = match captures.name("vendor") {
                        None => todo!("vendor"),
                        Some(value) => value.as_str().to_owned(),
                    };

                    let dep: String = match captures.name("dep") {
                        None => todo!("dep"),
                        Some(value) => value.as_str().to_owned(),
                    };

                    let major: u16 = match captures.name("major") {
                        None => todo!("major"),
                        Some(value) => match value.as_str().parse() {
                            Err(_) => todo!("major"),
                            Ok(value) => value,
                        }
                    };

                    let minor: u16 = match captures.name("minor") {
                        None => todo!("minor"),
                        Some(value) => match value.as_str().parse() {
                            Err(_) => todo!("minor"),
                            Ok(value) => value,
                        }
                    };

                    let patch: u16 = match captures.name("patch") {
                        None => todo!("patch"),
                        Some(value) => match value.as_str().parse() {
                            Err(_) => todo!("patch"),
                            Ok(value) => value,
                        }
                    };

                    PluginRef::new(vendor, dep, Semver::new(major, minor, patch))
                }
            }
        }
    }

    pub async fn execute(&self) {

    }
}

#[derive(Debug)]
pub struct PluginRef {
    pub dep: String,
    pub vendor: String,
    pub version: Semver,
}

impl PluginRef {
    fn new(vendor: String, dep: String, version: Semver) -> Self {
        Self { vendor, dep, version }
    }
}

fn find_pipelines_into(entries: &mut Vec<DirEntry>, path: impl AsRef<Path>) {
    let dir: ReadDir = match read_dir(path) {
        Err(error) => panic!("{:?}", error),
        Ok(value) => value,
    };

    for entry in dir {
        let entry: DirEntry = match entry {
            Err(error) => panic!("{:?}", error),
            Ok(value) => value,
        };

        let meta: Metadata = match entry.metadata() {
            Err(error) => panic!("{:?}", error),
            Ok(value) => value,
        };

        if meta.is_dir() {
            find_pipelines_into(entries, entry.path());
        }

        if meta.is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext.eq_ignore_ascii_case("pipeline") {
                    entries.push(entry);
                }
            }
        }
    }
}

async fn parse_pipelines_into(pipelines: &mut Vec<Pipeline>, entries: &Vec<DirEntry>) {
    for entry in entries {
        pipelines.push(Pipeline::open(entry.path()).await)
    }
}

pub async fn find_pipelines(path: impl AsRef<Path>) -> Vec<Pipeline> {
    let mut entries: Vec<DirEntry> = Vec::new();
    let mut pipelines: Vec<Pipeline> = Vec::new();

    find_pipelines_into(&mut entries, path);
    parse_pipelines_into(&mut pipelines, &entries).await;

    pipelines
}

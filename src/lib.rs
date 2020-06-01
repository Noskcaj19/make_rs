pub use anyhow::Result;
pub use std::path::{Path, PathBuf};

use anyhow::anyhow;
use std::process::ExitStatus;

pub trait Target {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter;
}

impl Target for &str {
    type Item = PathBuf;
    type IntoIter = std::vec::IntoIter<PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        vec![self.into()].into_iter()
    }
}

impl Target for glob::Paths {
    type Item = PathBuf;
    type IntoIter = std::vec::IntoIter<PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
            .flatten()
            .collect::<Vec<_>>()
            .into_iter()
    }
}

pub fn glob(pattern: &str) -> glob::Paths {
    glob::glob(pattern).unwrap()
}

pub fn create_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    Ok(std::fs::create_dir_all(path.as_ref())?)
}

fn is_newer(target: &Path, base: &Path) -> Result<bool> {
    let target_mtime = target.metadata()?.modified()?;
    let base_mtime = base.metadata()?.modified()?;

    Ok(target_mtime > base_mtime)
}

pub fn copy(src: impl Target<Item = impl AsRef<Path>>, dest: impl AsRef<Path>) -> Result<()> {
    for path in src.into_iter() {
        let dest = if dest.as_ref().is_dir() {
            dest.as_ref().join(
                path.as_ref()
                    .file_name()
                    .ok_or(anyhow!("Source has no filename and dest is a dir"))?,
            )
        } else {
            dest.as_ref().to_path_buf()
        };

        if is_newer(path.as_ref(), &dest).unwrap_or(true) {
            let _ = std::fs::copy(path, &dest);
        }
    }
    Ok(())
}

pub fn run(cmd: &str, args: &[&str]) -> Result<ExitStatus> {
    Ok(std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?)
}

pub fn env_or(env: &str, default: &str) -> String {
    std::env::var(env).unwrap_or(default.to_owned())
}

pub struct Maker {
    commands: Vec<(String, Box<dyn FnOnce() -> Result<()>>)>,
    default: Option<String>,
}

impl Maker {
    pub fn with() -> Maker {
        Maker {
            commands: vec![],
            default: None,
        }
    }

    pub fn default(mut self, name: &str) -> Self {
        self.default = Some(name.into());
        self
    }

    pub fn cmd(mut self, name: &str, cmd: impl FnOnce() -> Result<()> + 'static) -> Self {
        self.commands.push((name.into(), Box::new(cmd)));
        self
    }

    pub fn make(self) {
        let target = match std::env::args().skip(1).next() {
            Some(cmd) => cmd,
            None => match &self.default {
                Some(default) => default.clone(),
                None => {
                    eprintln!("No command was given");
                    return;
                }
            },
        };

        match self.commands.into_iter().find(|(cmd, _)| cmd == &target) {
            Some((_, func)) => {
                if let Err(err) = func() {
                    eprintln!("An error occurred:");
                    eprintln!("{}", err);
                }
            }
            None => {
                eprintln!("Unknown command: {}", target);
                return;
            }
        }
    }
}

pub trait PathHelper {
    fn to_string(&self) -> String;
}

impl PathHelper for Path {
    fn to_string(&self) -> String {
        self.to_string_lossy().to_string()
    }
}

pub trait ResultHelper<E> {
    fn ignore(self) -> Result<(), E>;
}

impl<T, E> ResultHelper<E> for std::result::Result<T, E> {
    fn ignore(self) -> Result<(), E> {
        self.map(|_| ())
    }
}

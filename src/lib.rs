pub use anyhow::Result;
pub use std::path::{Path, PathBuf};

use anyhow::anyhow;
use std::ffi::OsStr;
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

pub fn glob<P: AsRef<str>>(pattern: P) -> glob::Paths {
    glob::glob(pattern.as_ref()).unwrap()
}

pub fn create_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    Ok(std::fs::create_dir_all(path.as_ref())?)
}

fn is_newer<T: AsRef<Path>, B: AsRef<Path>>(target: T, base: B) -> Result<bool> {
    let target_mtime = target.as_ref().metadata()?.modified()?;
    let base_mtime = base.as_ref().metadata()?.modified()?;

    Ok(target_mtime > base_mtime)
}

pub fn copy<S, I, D>(src: S, dest: D) -> Result<()>
where
    S: Target<Item = I>,
    I: AsRef<Path>,
    D: AsRef<Path>,
{
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

pub fn run<I, S>(cmd: &str, args: I) -> Result<ExitStatus>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?)
}

pub fn env_or<K: AsRef<OsStr>, D: AsRef<str>>(env: K, default: D) -> String {
    std::env::var(env).unwrap_or(default.as_ref().to_owned())
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

    pub fn default<S: AsRef<str>>(mut self, name: S) -> Self {
        self.default = Some(name.as_ref().into());
        self
    }

    pub fn cmd<S: AsRef<str>>(
        mut self,
        name: S,
        cmd: impl FnOnce() -> Result<()> + 'static,
    ) -> Self {
        self.commands.push((name.as_ref().into(), Box::new(cmd)));
        self
    }

    pub fn make(mut self) {
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

        match self
            .commands
            .iter()
            .enumerate()
            .find(|(_, (cmd, _))| cmd == &target)
        {
            Some((i, _)) => {
                if let Err(err) = self.commands.remove(i).1() {
                    eprintln!("An error occurred:");
                    eprintln!("{}", err);
                }
            }
            None => {
                if &target == "help" {
                    eprintln!("Available commands:");
                    for (cmd, _) in self.commands {
                        eprintln!("  {}", cmd);
                    }
                } else {
                    eprintln!("Unknown command: {}", target);
                    return;
                }
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

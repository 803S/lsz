use std::{io, path::Path, process::Command};

pub trait ExternalOpenProvider {
    fn name(&self) -> &'static str;
    fn open(&self, path: &Path) -> io::Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemExternalOpenProvider;

impl ExternalOpenProvider for SystemExternalOpenProvider {
    fn name(&self) -> &'static str {
        "系统默认打开器"
    }

    fn open(&self, path: &Path) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(path).spawn()?;
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(path).spawn()?;
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", "start", "", &path.to_string_lossy()])
                .spawn()?;
        }
        Ok(())
    }
}

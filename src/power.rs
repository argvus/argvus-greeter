use thiserror::Error;
use zbus::blocking::{Connection, Proxy};

#[derive(Debug, Clone, Copy)]
pub enum PowerAction {
    Shutdown,
    Restart,
    Suspend,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shutdown => "Shutdown",
            Self::Restart => "Restart",
            Self::Suspend => "Suspend",
        }
    }
}

#[derive(Debug, Error)]
pub enum PowerError {
    #[error("could not connect to system bus: {0}")]
    Bus(#[from] zbus::Error),
}

pub fn request(action: PowerAction) -> Result<(), PowerError> {
    let connection = Connection::system()?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;

    match action {
        PowerAction::Shutdown => proxy.call::<_, _, ()>("PowerOff", &(true))?,
        PowerAction::Restart => proxy.call::<_, _, ()>("Reboot", &(true))?,
        PowerAction::Suspend => proxy.call::<_, _, ()>("Suspend", &(true))?,
    }

    Ok(())
}

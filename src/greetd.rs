use crate::session::Session;
use greetd_ipc::{
    AuthMessageType, ErrorType, Request, Response,
    codec::{Error as CodecError, SyncCodec},
};
use std::{
    env,
    os::unix::net::UnixStream,
    sync::mpsc::{Receiver, Sender},
    thread,
};
use thiserror::Error;
use tracing::{error, info, warn};

#[derive(Debug)]
pub enum Command {
    Begin { username: String, session: Session },
    AuthResponse(Option<String>),
}

#[derive(Debug, Clone)]
pub enum Event {
    Ready,
    AuthMessage(AuthPrompt),
    Info(String),
    Error(String),
    AuthFailed(String),
    SessionStarting,
    SessionStarted,
}

#[derive(Debug, Clone)]
pub struct AuthPrompt {
    pub message: String,
    pub secret: bool,
}

#[derive(Debug, Error)]
enum GreeterError {
    #[error("GREETD_SOCK is not set")]
    MissingSocket,
    #[error("could not connect to greetd socket: {0}")]
    Connect(std::io::Error),
    #[error("greetd IPC failed: {0}")]
    Ipc(#[from] CodecError),
}

pub fn spawn_worker(commands: Receiver<Command>, events: Sender<Event>) {
    thread::spawn(move || {
        let _ = events.send(Event::Ready);
        let mut state = WorkerState::Disconnected;

        while let Ok(command) = commands.recv() {
            match command {
                Command::Begin { username, session } => {
                    state = match begin_session(username, session, &events) {
                        Ok(state) => state,
                        Err(error) => {
                            warn!(%error, "could not begin greetd session");
                            let _ = events.send(Event::Error(error.to_string()));
                            WorkerState::Disconnected
                        }
                    };
                }
                Command::AuthResponse(response) => {
                    let WorkerState::Authenticating {
                        mut stream,
                        session,
                    } = state
                    else {
                        let _ =
                            events.send(Event::Error("No active authentication prompt.".into()));
                        state = WorkerState::Disconnected;
                        continue;
                    };

                    state = match send_auth_response(&mut stream, response, session, &events) {
                        Ok(next) => next,
                        Err(error) => {
                            warn!(%error, "authentication exchange failed");
                            let _ = events.send(Event::Error(error.to_string()));
                            WorkerState::Disconnected
                        }
                    };
                }
            }
        }
    });
}

enum WorkerState {
    Disconnected,
    Authenticating {
        stream: UnixStream,
        session: Session,
    },
}

fn begin_session(
    username: String,
    session: Session,
    events: &Sender<Event>,
) -> Result<WorkerState, GreeterError> {
    let socket = env::var("GREETD_SOCK").map_err(|_| GreeterError::MissingSocket)?;
    let mut stream = UnixStream::connect(socket).map_err(GreeterError::Connect)?;
    info!(%username, "connected to greetd");

    Request::CreateSession { username }.write_to(&mut stream)?;
    handle_response(&mut stream, session, events)
}

fn send_auth_response(
    stream: &mut UnixStream,
    response: Option<String>,
    session: Session,
    events: &Sender<Event>,
) -> Result<WorkerState, GreeterError> {
    Request::PostAuthMessageResponse { response }.write_to(stream)?;
    handle_response(stream, session, events)
}

fn handle_response(
    stream: &mut UnixStream,
    session: Session,
    events: &Sender<Event>,
) -> Result<WorkerState, GreeterError> {
    loop {
        match Response::read_from(stream)? {
            Response::Success => {
                info!(session = %session.id, "authentication succeeded");
                let _ = events.send(Event::SessionStarting);
                Request::StartSession {
                    cmd: session.command.clone(),
                    env: vec!["XDG_SESSION_TYPE=wayland".to_string()],
                }
                .write_to(stream)?;

                match Response::read_from(stream)? {
                    Response::Success => {
                        info!(session = %session.id, "session started");
                        let _ = events.send(Event::SessionStarted);
                    }
                    Response::Error {
                        error_type: _,
                        description,
                    } => {
                        error!(session = %session.id, "session start failed");
                        let _ = events.send(Event::Error(description));
                    }
                    Response::AuthMessage { .. } => {
                        let _ = events.send(Event::Error(
                            "greetd requested authentication after StartSession.".into(),
                        ));
                    }
                }
                return Ok(WorkerState::Disconnected);
            }
            Response::Error {
                error_type,
                description,
            } => {
                if matches!(error_type, ErrorType::AuthError) {
                    info!("authentication failed");
                    let _ = events.send(Event::AuthFailed(description));
                } else {
                    warn!(%description, "greetd returned an error");
                    let _ = events.send(Event::Error(description));
                }
                return Ok(WorkerState::Disconnected);
            }
            Response::AuthMessage {
                auth_message_type,
                auth_message,
            } => match auth_message_type {
                AuthMessageType::Secret => {
                    let _ = events.send(Event::AuthMessage(AuthPrompt {
                        message: auth_message,
                        secret: true,
                    }));
                    return Ok(WorkerState::Authenticating {
                        stream: stream.try_clone().map_err(GreeterError::Connect)?,
                        session,
                    });
                }
                AuthMessageType::Visible => {
                    let _ = events.send(Event::AuthMessage(AuthPrompt {
                        message: auth_message,
                        secret: false,
                    }));
                    return Ok(WorkerState::Authenticating {
                        stream: stream.try_clone().map_err(GreeterError::Connect)?,
                        session,
                    });
                }
                AuthMessageType::Info => {
                    let _ = events.send(Event::Info(auth_message));
                    Request::PostAuthMessageResponse { response: None }.write_to(stream)?;
                }
                AuthMessageType::Error => {
                    let _ = events.send(Event::Error(auth_message));
                    Request::PostAuthMessageResponse { response: None }.write_to(stream)?;
                }
            },
        }
    }
}

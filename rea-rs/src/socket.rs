use std::{
    fmt::Debug,
    sync::{
        mpsc::{channel, Receiver, RecvError, RecvTimeoutError, TryRecvError},
        Arc, Mutex,
    },
    thread::spawn,
    time::{Duration, Instant},
};

use anyhow::Error;
use serde::{Deserialize, Serialize};
use ws::{connect, listen};

use crate::ReaRsError;

/// Spawn the thread with a socket server.
///
/// Newcoming connections will be appended to the vector.
pub fn spawn_server<
    T: Debug
        + Serialize
        + for<'a> Deserialize<'a>
        + Send
        + 'static
        + std::clone::Clone,
>(
    url: impl AsRef<str>,
) -> Arc<Mutex<Vec<SocketHandle<T>>>> {
    let url = String::from(url.as_ref());
    let clients = Arc::new(Mutex::new(Vec::new()));
    let clcl = clients.clone();
    spawn(move || {
        listen(url, move |socket| {
            let (ssend, crcv) = channel::<T>();
            let client = SocketHandle {
                socket: socket.clone(),
                reciever: crcv,
            };
            clcl.lock().expect("can not lock clients").push(client);
            let ssend_cl = ssend.clone();
            move |msg| message_handler(msg, socket.clone(), ssend_cl.clone())
        })
        .expect("can not run server")
    });
    clients
}

/// Spawn the thread with the client socket and get its handle.
///
/// The function will wait for socket for 5 seconds.
pub fn spawn_client<
    T: Debug
        + Serialize
        + for<'a> Deserialize<'a>
        + Send
        + 'static
        + std::clone::Clone,
>(
    url: impl AsRef<str>,
) -> Result<SocketHandle<T>, Error> {
    let (ssend, crecv) = channel::<T>();
    let url = String::from(url.as_ref());
    let connection_result: Arc<Mutex<Option<Result<ws::Sender, ws::Error>>>> =
        Arc::new(Mutex::new(None));
    let cr_clone = connection_result.clone();
    spawn(move || {
        match connect(url, |socket| {
            let mut cr =
                cr_clone.lock().expect("can not lock connection_result");
            *cr = Some(Ok(socket.clone()));
            let ssend_cl = ssend.clone();
            move |msg| message_handler(msg, socket.clone(), ssend_cl.clone())
        }) {
            Ok(_) => (),
            Err(e) => {
                let mut cr =
                    cr_clone.lock().expect("can not lock connection_result");
                *cr = Some(Err(e));
            }
        };
    });
    let inst = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        let cr = connection_result
            .lock()
            .expect("can not lock connection_result");
        if let Some(r) = cr.as_ref() {
            match r {
                Err(e) => {
                    return Err(ReaRsError::Socket(format!("{e}")).into())
                }
                Ok(socket) => {
                    return Ok(SocketHandle {
                        socket: socket.clone(),
                        reciever: crecv,
                    })
                }
            }
        }
        if Instant::now() - inst > timeout {
            return Err(
                ReaRsError::Socket("connection timeout".to_string()).into()
            );
        }
    }
}

fn message_handler<
    T: Debug + Serialize + for<'a> Deserialize<'a> + Send + 'static + Clone,
>(
    msg: ws::Message,
    socket: ws::Sender,
    ssend_cl: std::sync::mpsc::Sender<T>,
) -> Result<(), ws::Error> {
    if let ws::Message::Text(s) = msg {
        match serde_json::from_str::<Message<T>>(&s) {
            Err(e) => {
                return Err(ws::Error {
                    kind: ws::ErrorKind::Custom(e.into()),
                    details: std::borrow::Cow::Borrowed(""),
                })
            }
            Ok(msg) => match msg {
                Message::Shutdown => {
                    socket.shutdown().expect("already shutdown")
                }
                Message::Message(m) => {
                    ssend_cl
                        .send(m.clone())
                        .expect("can not send message to main thread");
                }
            },
        }
    }
    Ok(())
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum Message<T: Debug + Serialize + Send> {
    Shutdown,
    Message(T),
}

/// Supports sending and recieving messages to the thread with the soket.
///
/// Works as for server, as well as for client.
#[derive(Debug)]
pub struct SocketHandle<T: Debug + Serialize + for<'a> Deserialize<'a> + Send>
{
    socket: ws::Sender,
    reciever: Receiver<T>,
}
impl<T: Debug + Serialize + for<'a> Deserialize<'a> + Send> SocketHandle<T> {
    /// Send message to other end.
    pub fn send(&self, msg: T) -> Result<(), ReaRsError> {
        let Ok(msg) = serde_json::to_string(&Message::Message(msg)) else {
            return Err(ReaRsError::Socket(
                "can not serialize message".to_string(),
            ));
        };
        self.socket
            .send(msg)
            .map_err(|e| ReaRsError::Socket(format!("Send error: {e}")))
    }
    /// recieve message from socket. Blocking.
    pub fn recv(&self) -> Result<T, RecvError> {
        self.reciever.recv()
    }
    /// recieve message from socket ir any
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.reciever.try_recv()
    }
    /// iter over available messages
    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        self.reciever.try_iter()
    }
    /// block, waiting for messages
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.reciever.iter()
    }
    /// wait for message
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<T, RecvTimeoutError> {
        self.reciever.recv_timeout(timeout)
    }
    /// shutdown the socket and send shutdown command to the other end(s)
    pub fn shutdown_all(&mut self) -> Result<(), ReaRsError> {
        let Ok(msg) = serde_json::to_string(&Message::<T>::Shutdown) else {
            return Err(ReaRsError::Socket(
                "can not serialize message".to_string(),
            ));
        };
        self.socket
            .send(msg)
            .map_err(|e| ReaRsError::Socket(format!("Send error: {e}")))?;
        self.socket
            .shutdown()
            .map_err(|e| ReaRsError::Socket(format!("Shutdown error: {e}")))
    }
    /// shutdown the socket.
    pub fn shutdown(&mut self) -> Result<(), ReaRsError> {
        self.socket
            .shutdown()
            .map_err(|e| ReaRsError::Socket(format!("Shutdown error: {e}")))
    }
}

//! Cancellable socket I/O with protocol timeouts longer than the native poll.
//! On Windows, shutdown through a clone can leave an existing recv blocked.

use super::ListenerLifetime;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CANCEL_POLL: Duration = Duration::from_millis(100);

pub(crate) struct CancelableStream {
    socket: TcpStream,
    lifetime: Arc<ListenerLifetime>,
    read_timeout: Option<Duration>,
}

impl CancelableStream {
    pub(crate) fn new(socket: TcpStream, lifetime: Arc<ListenerLifetime>) -> io::Result<Self> {
        let read_timeout = socket.read_timeout()?;
        socket.set_read_timeout(Some(CANCEL_POLL))?;
        socket.set_write_timeout(Some(CANCEL_POLL))?;
        Ok(Self {
            socket,
            lifetime,
            read_timeout,
        })
    }

    pub(crate) fn socket(&self) -> &TcpStream {
        &self.socket
    }

    pub(crate) fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        deadline(timeout)?;
        self.read_timeout = timeout;
        Ok(())
    }

    fn check_active(&self) -> io::Result<()> {
        if self.lifetime.is_active() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "browser integration stopped",
            ))
        }
    }

    pub(crate) fn peek(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_or_peek(buffer, true)
    }

    fn read_or_peek(&mut self, buffer: &mut [u8], peek: bool) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let deadline = deadline(self.read_timeout)?;
        loop {
            self.check_active()?;
            self.socket
                .set_read_timeout(Some(poll_duration(deadline)?))?;
            let result = if peek {
                self.socket.peek(buffer)
            } else {
                self.socket.read(buffer)
            };
            self.check_active()?;
            match result {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    continue
                }
                result => return result,
            }
        }
    }
}

fn deadline(timeout: Option<Duration>) -> io::Result<Option<Instant>> {
    timeout
        .map(|duration| {
            if duration.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "zero I/O timeout",
                ));
            }
            Instant::now().checked_add(duration).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "I/O timeout exceeds supported range",
                )
            })
        })
        .transpose()
}

fn poll_duration(deadline: Option<Instant>) -> io::Result<Duration> {
    let duration = deadline
        .map(|end| end.saturating_duration_since(Instant::now()))
        .unwrap_or(CANCEL_POLL);
    if duration.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "browser connection I/O timed out",
        ));
    }
    Ok(duration.min(CANCEL_POLL))
}

impl Read for CancelableStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_or_peek(buffer, false)
    }
}

impl Write for CancelableStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            self.check_active()?;
            let result = self.socket.write(buffer);
            self.check_active()?;
            match result {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    continue
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check_active()?;
        self.socket.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn pair() -> (CancelableStream, TcpStream, Arc<ListenerLifetime>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().unwrap();
        let lifetime = Arc::new(ListenerLifetime::default());
        let stream = CancelableStream::new(socket, Arc::clone(&lifetime)).unwrap();
        (stream, peer, lifetime)
    }

    #[test]
    fn fragmented_read_survives_multiple_native_poll_timeouts() {
        let (mut stream, mut peer, _) = pair();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        peer.write_all(b"h").unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            peer.write_all(b"i").unwrap();
        });
        let mut bytes = [0; 2];
        stream.read_exact(&mut bytes).unwrap();
        writer.join().unwrap();
        assert_eq!(&bytes, b"hi");
    }

    #[test]
    fn logical_timeout_expires_while_the_peer_stays_open() {
        let (mut stream, _peer, lifetime) = pair();
        stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .unwrap();
        let started = Instant::now();
        let error = stream.read(&mut [0]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(lifetime.is_active());
    }
}

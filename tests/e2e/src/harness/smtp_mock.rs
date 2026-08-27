//! A minimal SMTP server that accepts mail and remembers it.
//!
//! Neither this repo nor the astound template had any SMTP faking, and
//! `wiremock` cannot help — it speaks HTTP. The alternative to this file was
//! swapping a stub transport in behind the send path, which would have proved
//! the tool's control flow while skipping the part most likely to be wrong:
//! the actual `lettre` message construction, the envelope it derives, and the
//! headers that end up on the wire. So this is a real socket speaking real
//! SMTP, and `EmailService` connects to it exactly as it would to Resend.
//!
//! It implements only the commands lettre issues for a plain authenticated
//! send — EHLO, AUTH, MAIL FROM, RCPT TO, DATA, QUIT — and it is deliberately
//! credulous: any credential is accepted, because authentication is not what
//! the suite is testing. It never negotiates TLS, which is why the fixture
//! profile sets `smtp_security: plaintext`.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

// One message as the server received it.
#[derive(Debug, Clone)]
pub struct CapturedMail {
    // The envelope sender, from `MAIL FROM`.
    pub mail_from: String,
    // The envelope recipients, from each `RCPT TO`. This is what actually
    // decides who gets a copy — not the `To:` header — so it is what the
    // double-send assertions read.
    pub rcpt_to: Vec<String>,
    // The raw RFC5322 message, headers and body, as sent after `DATA`.
    pub data: String,
}

impl CapturedMail {
    // The value of a header, if present. Case-insensitive on the name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<String> {
        let prefix = format!("{}:", name.to_lowercase());
        self.data
            .lines()
            // Headers end at the first blank line; a body line that looks like
            // a header must not be mistaken for one.
            .take_while(|line| !line.trim().is_empty())
            .find(|line| line.to_lowercase().starts_with(&prefix))
            .map(|line| line[prefix.len()..].trim().to_owned())
    }

    // The body, everything after the header block.
    #[must_use]
    pub fn body(&self) -> String {
        self.data
            .split_once("\r\n\r\n")
            .or_else(|| self.data.split_once("\n\n"))
            .map_or_else(String::new, |(_, body)| body.trim_end().to_owned())
    }
}

pub struct SmtpMock {
    pub host: String,
    pub port: u16,
    received: Arc<Mutex<Vec<CapturedMail>>>,
}

impl SmtpMock {
    // Binds an ephemeral port and serves until the process ends.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral SMTP port");
        let port = listener
            .local_addr()
            .expect("read the bound SMTP address")
            .port();
        let received = Arc::new(Mutex::new(Vec::new()));

        let sink = Arc::clone(&received);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let sink = Arc::clone(&sink);
                tokio::spawn(async move {
                    // A session that goes wrong is not worth failing the whole
                    // listener over: the test's assertion on what was received
                    // is what reports the problem, and with a clearer message.
                    let _ = serve(stream, sink).await;
                });
            }
        });

        Self {
            host: "127.0.0.1".to_owned(),
            port,
            received,
        }
    }

    // Everything accepted so far.
    #[must_use]
    pub fn received(&self) -> Vec<CapturedMail> {
        self.received
            .lock()
            .expect("the capture list is not poisoned")
            .clone()
    }

    // How many messages have been accepted. The exactly-one-copy check.
    #[must_use]
    pub fn count(&self) -> usize {
        self.received().len()
    }
}

async fn serve(
    stream: tokio::net::TcpStream,
    sink: Arc<Mutex<Vec<CapturedMail>>>,
) -> std::io::Result<()> {
    let (read_half, mut write) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    write.write_all(b"220 e2e-smtp-mock ESMTP\r\n").await?;

    let mut mail_from = String::new();
    let mut rcpt_to: Vec<String> = Vec::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        let command = line.trim_end();
        let upper = command.to_uppercase();

        if upper.starts_with("EHLO") || upper.starts_with("HELO") {
            // AUTH has to be advertised: lettre is configured with credentials
            // and errors out rather than sending if no mechanism is offered.
            write
                .write_all(b"250-e2e-smtp-mock\r\n250-AUTH PLAIN LOGIN\r\n250 OK\r\n")
                .await?;
        } else if upper.starts_with("AUTH") {
            // Any credential passes. Authentication is not under test here,
            // and rejecting would only prove lettre reports a 535.
            if upper.starts_with("AUTH LOGIN") {
                write.write_all(b"334 VXNlcm5hbWU6\r\n").await?;
                line.clear();
                reader.read_line(&mut line).await?;
                write.write_all(b"334 UGFzc3dvcmQ6\r\n").await?;
                line.clear();
                reader.read_line(&mut line).await?;
            }
            write.write_all(b"235 authenticated\r\n").await?;
        } else if upper.starts_with("MAIL FROM") {
            mail_from = extract_address(command);
            rcpt_to.clear();
            write.write_all(b"250 OK\r\n").await?;
        } else if upper.starts_with("RCPT TO") {
            rcpt_to.push(extract_address(command));
            write.write_all(b"250 OK\r\n").await?;
        } else if upper.starts_with("DATA") {
            write
                .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                .await?;
            let data = read_data(&mut reader).await?;
            sink.lock()
                .expect("the capture list is not poisoned")
                .push(CapturedMail {
                    mail_from: std::mem::take(&mut mail_from),
                    rcpt_to: std::mem::take(&mut rcpt_to),
                    data,
                });
            write.write_all(b"250 OK: queued\r\n").await?;
        } else if upper.starts_with("QUIT") {
            write.write_all(b"221 Bye\r\n").await?;
            return Ok(());
        } else if upper.starts_with("RSET") {
            mail_from.clear();
            rcpt_to.clear();
            write.write_all(b"250 OK\r\n").await?;
        } else if upper.starts_with("NOOP") {
            write.write_all(b"250 OK\r\n").await?;
        } else {
            write.write_all(b"502 not implemented\r\n").await?;
        }
    }
}

// Reads the DATA block up to the lone `.` terminator, undoing dot-stuffing.
async fn read_data(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> std::io::Result<String> {
    let mut data = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(data);
        }
        if line.trim_end() == "." {
            return Ok(data);
        }
        // RFC5321 dot-stuffing: a body line that began with '.' was sent with
        // an extra one. Leaving it in would corrupt any body starting a line
        // with a period.
        let unstuffed = line.strip_prefix("..").map_or(line.as_str(), |rest| {
            data.push('.');
            rest
        });
        data.push_str(unstuffed);
    }
}

// Pulls the address out of `MAIL FROM:<a@b.io>` / `RCPT TO:<a@b.io>`.
fn extract_address(command: &str) -> String {
    command
        .split_once('<')
        .and_then(|(_, rest)| rest.split_once('>'))
        .map_or_else(
            || {
                command
                    .split_once(':')
                    .map_or_else(String::new, |(_, rest)| rest.trim().to_owned())
            },
            |(address, _)| address.to_owned(),
        )
}

//! Blocking IMAP-over-rustls transport for the ingestion job: open a session,
//! fetch unseen messages without flagging them, and mark captured UIDs seen.

use std::net::TcpStream;
use std::sync::Arc;

use crate::error::KnowledgeJobError;

#[derive(Debug, Clone)]
pub(crate) struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub mailbox: String,
    pub max_batch: usize,
}

#[derive(Debug)]
pub(crate) struct FetchedMessage {
    pub uid: u32,
    pub raw: Vec<u8>,
}

type TlsSession = imap::Session<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>;

fn open_session(config: &ImapConfig) -> Result<TlsSession, KnowledgeJobError> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // Why: the workspace links more than one rustls crypto backend, so the
    // process-default provider is ambiguous; name ring explicitly.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| KnowledgeJobError::Imap(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = rustls::pki_types::ServerName::try_from(config.host.clone())
        .map_err(|e| KnowledgeJobError::Config(format!("invalid IMAP host: {e}")))?;
    let connection = rustls::ClientConnection::new(Arc::new(tls_config), server_name)
        .map_err(|e| KnowledgeJobError::Imap(e.to_string()))?;
    let tcp = TcpStream::connect((config.host.as_str(), config.port)).map_err(|e| {
        KnowledgeJobError::Imap(format!("connect {}:{}: {e}", config.host, config.port))
    })?;
    let stream = rustls::StreamOwned::new(connection, tcp);

    let client = imap::Client::new(stream);
    let mut session = client
        .login(&config.user, &config.password)
        .map_err(|(e, _)| KnowledgeJobError::Imap(format!("login as {}: {e}", config.user)))?;
    session
        .select(&config.mailbox)
        .map_err(|e| KnowledgeJobError::Imap(format!("select {}: {e}", config.mailbox)))?;
    Ok(session)
}

// Why: BODY.PEEK[] keeps the \Seen flag untouched, so a crash between fetch
// and database commit leaves the mailbox exactly as it was.
pub(crate) fn fetch_unseen(config: &ImapConfig) -> Result<Vec<FetchedMessage>, KnowledgeJobError> {
    let mut session = open_session(config)?;

    let mut uids: Vec<u32> = session
        .uid_search("UNSEEN")
        .map_err(|e| KnowledgeJobError::Imap(format!("search UNSEEN: {e}")))?
        .into_iter()
        .collect();
    uids.sort_unstable();
    uids.truncate(config.max_batch);

    if uids.is_empty() {
        close(session);
        return Ok(Vec::new());
    }

    let set = uid_set(&uids);
    let fetches = session
        .uid_fetch(&set, "(UID BODY.PEEK[])")
        .map_err(|e| KnowledgeJobError::Imap(format!("fetch {set}: {e}")))?;

    let mut messages = Vec::with_capacity(uids.len());
    for fetch in &*fetches {
        let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) else {
            continue;
        };
        messages.push(FetchedMessage {
            uid,
            raw: body.to_vec(),
        });
    }

    close(session);
    Ok(messages)
}

pub(crate) fn mark_seen(config: &ImapConfig, uids: &[u32]) -> Result<(), KnowledgeJobError> {
    let mut session = open_session(config)?;
    let set = uid_set(uids);
    session
        .uid_store(&set, "+FLAGS (\\Seen)")
        .map_err(|e| KnowledgeJobError::Imap(format!("store {set}: {e}")))?;
    close(session);
    Ok(())
}

// Why: logout failure only leaks a server-side session that Gmail reaps;
// the fetch/store work is already done, so log and move on.
fn close(mut session: TlsSession) {
    if let Err(e) = session.logout() {
        tracing::warn!(error = %e, "imap logout failed");
    }
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

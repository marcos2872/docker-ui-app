use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;

use crate::ssh::{CommandResult, SshConnection, SshError, SshServerInfo};

#[allow(dead_code)]
pub struct SshClient {
    session: Option<Session>,
    connection_info: Option<SshConnection>,
}

#[allow(dead_code)]
impl SshClient {
    pub fn new() -> Self {
        Self {
            session: None,
            connection_info: None,
        }
    }

    pub async fn connect(&mut self, connection: SshConnection) -> Result<(), SshError> {
        let tcp = TcpStream::connect(format!("{}:{}", connection.host, connection.port))
            .map_err(SshError::from)?;

        let mut sess = Session::new().map_err(SshError::from)?;
        sess.set_tcp_stream(tcp);
        sess.handshake().map_err(SshError::from)?;

        if !connection.password.is_empty() {
            sess.userauth_password(&connection.username, &connection.password)
                .map_err(SshError::from)?;
        } else if let Some(private_key) = &connection.private_key {
            let private_key_path = Path::new(private_key);
            sess.userauth_pubkey_file(
                &connection.username,
                None,
                private_key_path,
                connection.passphrase.as_deref(),
            )
            .map_err(SshError::from)?;
        } else {
            return Err(SshError::InvalidCredentials(
                "Either password or private key must be provided".to_string(),
            ));
        }

        if !sess.authenticated() {
            return Err(SshError::AuthenticationFailed(
                "Authentication failed".to_string(),
            ));
        }

        self.session = Some(sess);
        self.connection_info = Some(connection);

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some() && self.connection_info.is_some()
    }

    pub fn disconnect(&mut self) {
        if let Some(session) = &mut self.session {
            let _ = session.disconnect(None, "Disconnecting", None);
        }
        self.session = None;
        self.connection_info = None;
    }

    pub async fn execute_command(&self, command: &str) -> Result<CommandResult, SshError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| SshError::ConnectionFailed("Not connected".to_string()))?;

        let mut channel = session.channel_session().map_err(SshError::from)?;

        channel.exec(command).map_err(SshError::from)?;

        let mut stdout = String::new();
        channel
            .read_to_string(&mut stdout)
            .map_err(|e| SshError::CommandExecutionFailed(e.to_string()))?;

        let mut stderr = String::new();
        channel
            .stderr()
            .read_to_string(&mut stderr)
            .map_err(|e| SshError::CommandExecutionFailed(e.to_string()))?;

        channel.wait_close().map_err(SshError::from)?;

        let exit_code = channel.exit_status().map_err(SshError::from)?;

        Ok(CommandResult {
            command: command.to_string(),
            stdout,
            stderr,
            exit_code,
            success: exit_code == 0,
        })
    }

    pub async fn get_server_info(&self) -> Result<SshServerInfo, SshError> {
        let hostname_result = self.execute_command("hostname").await?;
        let uptime_result = self.execute_command("uptime").await?;
        let memory_result = self.execute_command("free -h").await?;
        let cpu_result = self.execute_command("top -bn1 | grep 'Cpu(s)' | sed 's/.*, *\\([0-9.]*\\)%* id.*/\\1/' | awk '{print 100 - $1\"%\"}'").await?;
        let disk_result = self.execute_command("df -h /").await?;

        Ok(SshServerInfo {
            hostname: hostname_result.stdout.trim().to_string(),
            uptime: uptime_result.stdout.trim().to_string(),
            memory_usage: memory_result.stdout.trim().to_string(),
            cpu_usage: cpu_result.stdout.trim().to_string(),
            disk_usage: disk_result.stdout.trim().to_string(),
        })
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<(), SshError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| SshError::ConnectionFailed("Not connected".to_string()))?;

        let local_file = std::fs::File::open(local_path)
            .map_err(|e| SshError::FileTransferFailed(format!("Cannot open local file: {}", e)))?;

        let metadata = local_file.metadata().map_err(|e| {
            SshError::FileTransferFailed(format!("Cannot read file metadata: {}", e))
        })?;

        let mut remote_file = session
            .scp_send(Path::new(remote_path), 0o644, metadata.len(), None)
            .map_err(SshError::from)?;

        let mut local_content = Vec::new();
        std::fs::File::open(local_path)
            .and_then(|mut f| f.read_to_end(&mut local_content))
            .map_err(|e| SshError::FileTransferFailed(format!("Cannot read local file: {}", e)))?;

        remote_file.write_all(&local_content).map_err(|e| {
            SshError::FileTransferFailed(format!("Cannot write to remote file: {}", e))
        })?;

        remote_file.send_eof().map_err(SshError::from)?;

        remote_file.wait_eof().map_err(SshError::from)?;

        remote_file.close().map_err(SshError::from)?;

        remote_file.wait_close().map_err(SshError::from)?;

        Ok(())
    }

    pub async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<(), SshError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| SshError::ConnectionFailed("Not connected".to_string()))?;

        let (mut remote_file, _stat) = session
            .scp_recv(Path::new(remote_path))
            .map_err(SshError::from)?;

        let mut contents = Vec::new();
        remote_file
            .read_to_end(&mut contents)
            .map_err(|e| SshError::FileTransferFailed(format!("Cannot read remote file: {}", e)))?;

        remote_file.send_eof().map_err(SshError::from)?;

        remote_file.wait_eof().map_err(SshError::from)?;

        remote_file.close().map_err(SshError::from)?;

        remote_file.wait_close().map_err(SshError::from)?;

        std::fs::write(local_path, contents)
            .map_err(|e| SshError::FileTransferFailed(format!("Cannot write local file: {}", e)))?;

        Ok(())
    }

    pub fn get_connection_info(&self) -> Option<&SshConnection> {
        self.connection_info.as_ref()
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

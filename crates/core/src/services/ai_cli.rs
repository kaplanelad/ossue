use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to spawn claude CLI. Is it installed?")]
    Spawn(#[source] std::io::Error),

    #[error("Failed to capture {0} from spawned process")]
    MissingPipe(&'static str),

    #[error("Failed to send to CLI stdin")]
    Send(#[from] tokio::sync::mpsc::error::SendError<String>),
}

type Result<T> = std::result::Result<T, Error>;

pub struct AiCliService {
    child: Option<Child>,
    stdin_tx: Option<mpsc::Sender<String>>,
}

impl Default for AiCliService {
    fn default() -> Self {
        Self::new()
    }
}

impl AiCliService {
    pub fn new() -> Self {
        Self {
            child: None,
            stdin_tx: None,
        }
    }

    pub async fn spawn(&mut self, cwd: PathBuf, output_tx: mpsc::Sender<String>) -> Result<()> {
        self.kill().await;

        let mut child = Command::new("claude")
            .current_dir(&cwd)
            .arg("--print")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Error::Spawn)?;

        let stdout = child.stdout.take().ok_or(Error::MissingPipe("stdout"))?;
        let stderr = child.stderr.take().ok_or(Error::MissingPipe("stderr"))?;

        // Spawn stdout reader
        let out_tx = output_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if out_tx.send(line).await.is_err() {
                    break;
                }
            }
        });

        // Spawn stderr reader
        let err_tx = output_tx;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if err_tx.send(format!("[stderr] {line}")).await.is_err() {
                    break;
                }
            }
        });

        // Stdin writer channel
        let stdin = child.stdin.take().ok_or(Error::MissingPipe("stdin"))?;
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);

        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = stdin_rx.recv().await {
                if stdin.write_all(msg.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        self.child = Some(child);
        self.stdin_tx = Some(stdin_tx);

        Ok(())
    }

    pub async fn send(&self, message: &str) -> Result<()> {
        if let Some(tx) = &self.stdin_tx {
            tx.send(message.to_string()).await?;
        }
        Ok(())
    }

    pub async fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        self.stdin_tx = None;
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

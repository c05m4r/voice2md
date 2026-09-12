//! Worker pool: encola `Job`s y los procesa sin bloquear al productor (bot).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, error};

use crate::domain::errors::DomainError;
use crate::domain::value::{FormattedOutputStyle, LanguageIso};
use crate::ports::inbound::{AudioInput, ProcessAudioRequest, ProcessOutcome};

/// Unidad de trabajo encolada por un adaptador inbound.
pub struct Job {
    pub input: AudioInput,
    pub language: LanguageIso,
    pub style: FormattedOutputStyle,
    pub duration_hint: Option<u32>,
    pub reply: oneshot::Sender<Result<ProcessOutcome, DomainError>>,
}

/// Pool de workers que consume la cola y ejecuta el caso de uso central.
pub struct WorkerPool {
    tx: mpsc::Sender<Job>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    /// Crea un pool con `workers` tareas y un tamaño de cola acotado.
    pub fn spawn(
        processor: Arc<dyn ProcessAudioRequest>,
        workers: usize,
        queue_capacity: usize,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<Job>(queue_capacity);
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let mut handles = Vec::with_capacity(workers);

        for i in 0..workers {
            let processor = Arc::clone(&processor);
            let rx = Arc::clone(&rx);
            handles.push(tokio::spawn(async move {
                debug!(worker = i, "worker started");
                loop {
                    let job = {
                        let mut guard = rx.lock().await;
                        match guard.recv().await {
                            Some(job) => job,
                            None => break,
                        }
                    };
                    let result = processor
                        .process(job.input, job.language, job.style, job.duration_hint)
                        .await;
                    if job.reply.send(result).is_err() {
                        error!("receiver dropped before worker replied");
                    }
                }
            }));
        }

        Self { tx, handles }
    }

    /// Encadena un job y devuelve el receptor del resultado.
    pub async fn submit(
        &self,
        input: AudioInput,
        language: LanguageIso,
        style: FormattedOutputStyle,
        duration_hint: Option<u32>,
    ) -> Result<ProcessOutcome, SubmitError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job = Job {
            input,
            language,
            style,
            duration_hint,
            reply: reply_tx,
        };

        self.tx.send(job).await.map_err(|_| SubmitError::Shutdown)?;

        // `await` con timeout para no colgar al productor indefinidamente.
        tokio::time::timeout(Duration::from_secs(120), reply_rx)
            .await
            .map_err(|_| SubmitError::Timeout)?
            .map_err(|_| SubmitError::Cancelled)?
            .map_err(SubmitError::Processing)
    }

    /// Cierra la cola; los workers terminan al vaciarla.
    pub async fn shutdown(self) {
        drop(self.tx);
        for h in self.handles {
            let _ = h.await;
        }
    }
}

/// Errores de envío/espera en el pool.
#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error("worker pool is shut down")]
    Shutdown,
    #[error("processing timed out")]
    Timeout,
    #[error("processing was cancelled")]
    Cancelled,
    #[error("processing failed: {0}")]
    Processing(#[from] DomainError),
}

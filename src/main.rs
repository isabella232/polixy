use anyhow::{Context, Result};
use futures::{future, prelude::*};
use polixy::index;
use structopt::StructOpt;
use tracing::{debug, info, instrument};

#[derive(Debug, StructOpt)]
#[structopt(name = "polixy", about = "A policy resource prototype")]
enum Command {
    Controller {
        #[structopt(short, long, default_value = "8910")]
        port: u16,
        #[structopt(long, default_value = "cluster.local")]
        identity_domain: String,
    },
    Client {
        #[structopt(long, default_value = "http://127.0.0.1:8910")]
        server: String,
        #[structopt(subcommand)]
        command: ClientCommand,
    },
}

#[derive(Debug, StructOpt)]
enum ClientCommand {
    Watch {
        #[structopt(short, long, default_value = "default")]
        namespace: String,
        pod: String,
        port: u16,
    },
    Get {
        #[structopt(short, long, default_value = "default")]
        namespace: String,
        pod: String,
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    match Command::from_args() {
        Command::Controller {
            port,
            identity_domain,
        } => {
            let (drain_tx, drain_rx) = linkerd_drain::channel();

            let client = kube::Client::try_default()
                .await
                .context("failed to initialize kubernetes client")?;
            let (index, index_task) = index::run(client);

            let index_task = tokio::spawn(index_task);
            let grpc = tokio::spawn(grpc(port, index, drain_rx, identity_domain));

            tokio::select! {
                _ = shutdown(drain_tx) => Ok(()),
                res = grpc => match res {
                    Ok(res) => res.context("grpc server failed"),
                    Err(e) if e.is_cancelled() => Ok(()),
                    Err(e) => Err(e).context("grpc server panicked"),
                },
                res = index_task => match res {
                    Ok(e) => Err(e).context("indexer failed"),
                    Err(e) if e.is_cancelled() => Ok(()),
                    Err(e) => Err(e).context("indexer panicked"),
                },
            }
        }

        Command::Client { server, command } => match command {
            ClientCommand::Watch {
                namespace,
                pod,
                port,
            } => {
                let mut client = polixy::grpc::Client::connect(server).await?;
                let mut updates = client.watch_inbound(namespace, pod, port).await?;
                while let Some(config) = updates.try_next().await? {
                    println!("{:#?}", config);
                }
                eprintln!("Stream closed");
                Ok(())
            }

            ClientCommand::Get {
                namespace,
                pod,
                port,
            } => {
                let mut client = polixy::grpc::Client::connect(server).await?;
                let mut updates = client.watch_inbound(namespace, pod, port).await?;
                if let Some(config) = updates.try_next().await? {
                    println!("{:#?}", config);
                } else {
                    eprintln!("No configuration read");
                }
                Ok(())
            }
        },
    }
}

#[instrument(skip(index, drain))]
async fn grpc(
    port: u16,
    index: index::Handle,
    drain: linkerd_drain::Watch,
    identity_domain: String,
) -> Result<()> {
    let addr = ([0, 0, 0, 0], port).into();
    let server = polixy::grpc::Server::new(index, drain.clone(), identity_domain);
    let (close_tx, close_rx) = tokio::sync::oneshot::channel();
    tokio::pin! {
        let srv = server.serve(addr, close_rx.map(|_| {}));
    }
    info!(%addr, "gRPC server listening");
    tokio::select! {
        res = (&mut srv) => res?,
        handle = drain.signaled() => {
            let _ = close_tx.send(());
            handle.release_after(srv).await?
        }
    }
    Ok(())
}

async fn shutdown(drain: linkerd_drain::Signal) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            debug!("Received ctrl-c");
        },
        _ = sigterm() => {
            debug!("Received SIGTERM");
        }
    }
    info!("Shutting down");
    drain.drain().await;
}

async fn sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    match signal(SignalKind::terminate()) {
        Ok(mut term) => term.recv().await,
        _ => future::pending().await,
    };
}

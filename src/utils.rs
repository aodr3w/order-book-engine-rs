//utils for graceful shutdown that can be used on the
//any module in the project
use tokio::signal;
use tokio_util::sync::CancellationToken;

pub fn shutdown_token() -> CancellationToken {
    let token = CancellationToken::new();
    let tc = token.clone();
    //spawn once to listen for ctrl-c
    tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("failed to install ctrl+C handler");
        tc.cancel();
    });
    token
}

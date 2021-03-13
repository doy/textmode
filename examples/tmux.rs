#[cfg(feature = "async")]
mod tmux_impl;

#[cfg(feature = "async")]
async fn async_main(ex: &smol::Executor<'_>) {
    let tmux = tmux_impl::Tmux::new().await;
    tmux.run(ex).await;
}

#[cfg(feature = "async")]
fn main() {
    let ex = smol::Executor::new();
    smol::block_on(async { async_main(&ex).await })
}

#[cfg(not(feature = "async"))]
fn main() {
    panic!("tmux example requires feature async")
}

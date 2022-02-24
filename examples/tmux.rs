#[cfg(feature = "async")]
mod tmux_impl;

#[cfg(feature = "async")]
#[tokio::main]
async fn main() {
    let tmux = tmux_impl::Tmux::new().await;
    tmux.run().await;
}

#[cfg(not(feature = "async"))]
fn main() {
    panic!("tmux example requires feature async")
}

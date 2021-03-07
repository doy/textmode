use textmode::TextmodeExt as _;

async fn run(tm: &mut textmode::r#async::Textmode) -> std::io::Result<()> {
    tm.move_to(5, 5);
    tm.write_str("foo");
    smol::Timer::after(std::time::Duration::from_secs(2)).await;
    tm.refresh().await?;
    smol::Timer::after(std::time::Duration::from_secs(2)).await;

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_to(11, 11);
    tm.set_fgcolor(vt100::Color::Default);
    tm.write_str("baz");
    smol::Timer::after(std::time::Duration::from_secs(2)).await;
    tm.refresh().await?;
    smol::Timer::after(std::time::Duration::from_secs(2)).await;
    Ok(())
}

fn main() {
    smol::block_on(async {
        let mut tm = textmode::r#async::Textmode::new().await.unwrap();
        let e = run(&mut tm).await;
        tm.cleanup().await.unwrap();
        e.unwrap();
    });
}
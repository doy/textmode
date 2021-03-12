use textmode::Textmode as _;

async fn run(
    tm: &mut textmode::Output,
    input: &mut textmode::Input,
) -> textmode::Result<()> {
    tm.move_to(5, 5);
    tm.write_str("foo");
    input.read_key().await?;
    tm.refresh().await?;
    input.read_key().await?;

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_to(11, 11);
    tm.set_fgcolor(vt100::Color::Default);
    tm.write_str("baz");
    input.read_key().await?;
    tm.refresh().await?;
    input.read_key().await?;
    Ok(())
}

fn main() {
    smol::block_on(async {
        let mut tm = textmode::Output::new().await.unwrap();
        let mut input = textmode::Input::new().await.unwrap();
        let e = run(&mut tm, &mut input).await;
        e.unwrap();
    });
}

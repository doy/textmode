use textmode::Textmode as _;

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> textmode::Result<()> {
    let mut input = textmode::Input::new().await?;
    let mut tm = textmode::Output::new().await?;
    tm.move_to(5, 5);
    tm.write_str("foo");
    input.read_key().await?;
    tm.refresh().await?;
    input.read_key().await?;

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_relative(3, 0);
    tm.set_fgcolor(textmode::Color::Default);
    tm.write_str("baz");
    input.read_key().await?;
    tm.refresh().await?;
    input.read_key().await?;
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    let mut input = textmode::blocking::Input::new().unwrap();
    let mut tm = textmode::blocking::Output::new().unwrap();

    tm.move_to(5, 5);
    tm.write_str("foo");
    input.read_key().unwrap();
    tm.refresh().unwrap();
    input.read_key().unwrap();

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_relative(3, 0);
    tm.set_fgcolor(textmode::Color::Default);
    tm.write_str("baz");
    input.read_key().unwrap();
    tm.refresh().unwrap();
    input.read_key().unwrap();
}

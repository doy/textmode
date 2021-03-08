fn main() {
    let (mut input, _raw) = textmode::blocking::Input::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--disable-utf8" => input.parse_utf8(false),
            "--disable-ctrl" => input.parse_ctrl(false),
            "--disable-meta" => input.parse_meta(false),
            "--disable-special-keys" => input.parse_special_keys(false),
            "--disable-single" => input.parse_single(false),
            _ => panic!("unknown arg {}", arg),
        }
    }

    loop {
        let key = input.read_key().unwrap();
        if let Some(key) = key {
            print!("{:?}: ", key);
            let bytes = key.into_bytes();
            print!("{:?}\r\n", bytes);
            if bytes.contains(&3) {
                break;
            }
        } else {
            break;
        }
    }
}

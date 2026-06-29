module main

enum OptionI32 {
    Some(i32 value);
    None;
}

enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

i32 main() {
    OptionI32 x = Some(42);
    i32 from_option = match x {
        Some(v) => v,
        None => 0,
    };

    ResultI32 r = Ok(from_option);
    return match r {
        Ok(v) => v,
        Err(_) => 0,
    };
}

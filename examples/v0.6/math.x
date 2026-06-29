module math

pub enum ResultI32 {
    Ok(i32 value);
    Err(i32 code);
}

pub ResultI32 divide(i32 a, i32 b) {
    if b == 0 {
        return Err(1);
    }
    return Ok(a / b);
}

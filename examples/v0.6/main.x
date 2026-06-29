module main
import math

i32 main() {
    return match math.divide(10, 2) {
        Ok(v) => v,
        Err(_) => 0,
    };
}

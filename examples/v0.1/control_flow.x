module control_flow

bool is_answer(i32 value) {
    return value == 42;
}

i32 main() {
    i32 x = 10;
    x = x * 4 + 2;

    if is_answer(x) && !(x < 0) {
        return x;
    } else {
        return 1;
    }
}
